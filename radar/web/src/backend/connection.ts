import { EventEmitter } from "../utils/ee";
import { C2SMessage, HandshakeProtocolV2, RadarState, S2CMessage } from "./definitions";

/** Aurora protocol version — must match `radar/shared/src/protocol.rs`. */
export const RADAR_PROTOCOL_VERSION = 3;

export type SubscriberClientState =
    | { state: "new" }
    | { state: "connecting" }
    | { state: "handshaking" }
    | { state: "initializing" }
    | { state: "connected" }
    | { state: "disconnected" }
    | { state: "reconnecting"; attempt: number }
    | { state: "failed"; reason: string };

export interface SubscriberClientEvents {
    state_changed: SubscriberClientState;
    "radar.state": RadarState;
    "view.count": number;
}

/**
 * Simple EMA-based update statistics tracker for the radar state stream.
 */
export class UpdateStatistics {
    private readonly alpha: number = 0.92;
    private readonly historySize: number = 120;

    private timestampLastUpdate: number | null = null;

    private history: number[] = [];
    private historyIndex: number = 0;
    private movingAverage: number = 0;
    private minInterval: number = Infinity;
    private maxInterval: number = 0;

    constructor() {
        this.history = new Array(this.historySize).fill(0);
    }

    public logUpdate(): void {
        const now = performance.now();
        if (this.timestampLastUpdate === null) {
            this.timestampLastUpdate = now;
            return;
        }
        const passed = now - this.timestampLastUpdate;
        this.timestampLastUpdate = now;

        this.history[this.historyIndex] = passed;
        this.historyIndex = (this.historyIndex + 1) % this.historySize;

        this.movingAverage = this.movingAverage === 0
            ? passed
            : this.movingAverage * this.alpha + passed * (1 - this.alpha);

        if (passed < this.minInterval) this.minInterval = passed;
        if (passed > this.maxInterval) this.maxInterval = passed;
    }

    public reset(): void {
        this.timestampLastUpdate = null;
        this.movingAverage = 0;
        this.minInterval = Infinity;
        this.maxInterval = 0;
        this.historyIndex = 0;
        this.history.fill(0);
    }

    public getAverageInterval(): number {
        return this.movingAverage;
    }

    public getMinInterval(): number {
        return this.minInterval === Infinity ? 0 : this.minInterval;
    }

    public getMaxInterval(): number {
        return this.maxInterval;
    }

    public getUps(): number {
        return this.movingAverage > 0 ? 1000 / this.movingAverage : 0;
    }

    public getHistory(): Readonly<number[]> {
        return this.history;
    }
}

export type ClientVersion = typeof RADAR_PROTOCOL_VERSION;

/**
 * SubscriberClient manages a single WebSocket session against an Aurora radar
 * server. It handles protocol handshaking, automatic reconnection with
 * exponential back-off, state transitions, and per-message dispatch.
 */
export class SubscriberClient {
    public readonly events: EventEmitter<SubscriberClientEvents>;
    public readonly stateUpdateStatistics: UpdateStatistics = new UpdateStatistics();

    private currentState: SubscriberClientState = { state: "new" };
    private connection: WebSocket | null = null;
    private currentRadarState: RadarState | null = null;
    private reconnectAttempt: number = 0;
    private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
    private shouldReconnect: boolean = true;

    /**
     * Outbound queue: messages pushed before the socket is OPEN are buffered
     * here and flushed as soon as the socket is ready.
     */
    private pendingOutbound: string[] = [];

    private readonly commandHandler: {
        [T in S2CMessage["type"]]?: (payload: Extract<S2CMessage, { type: T }>["payload"]) => void;
    } = {};

    constructor(public readonly targetAddress: string) {
        this.events = new EventEmitter();
        this.events.setMaxListeners(60);

        this.commandHandler["response-error"] = (payload) => {
            this.updateState({ state: "failed", reason: payload.error });
            this.shutdownSocket(true);
        };
        this.commandHandler["response-session-invalid-id"] = () => {
            this.updateState({ state: "failed", reason: "Session does not exist or is offline" });
            this.shutdownSocket(true);
        };
        this.commandHandler["response-subscribe-success"] = () => {
            this.reconnectAttempt = 0;
            this.updateState({ state: "connected" });
        };
        this.commandHandler["notify-radar-state"] = (payload) => {
            this.currentRadarState = payload.state;
            this.stateUpdateStatistics.logUpdate();
            this.events.emit("radar.state", payload.state);
        };
        this.commandHandler["notify-view-count"] = (payload) => {
            this.events.emit("view.count", payload.viewers);
        };
        this.commandHandler["notify-session-closed"] = () => {
            this.updateState({ state: "disconnected" });
            this.shutdownSocket(false);
        };
    }

    public getCurrentRadarState(): Readonly<RadarState> | null {
        return this.currentRadarState;
    }

    public getState(): Readonly<SubscriberClientState> {
        return this.currentState;
    }

    private updateState(newState: SubscriberClientState): void {
        this.currentState = newState;
        this.events.emit("state_changed", newState);
    }

    private shutdownSocket(doNotReconnect: boolean): void {
        if (doNotReconnect) {
            this.shouldReconnect = false;
        }
        if (this.reconnectTimer !== null) {
            clearTimeout(this.reconnectTimer);
            this.reconnectTimer = null;
        }
        if (!this.connection) return;

        try {
            this.connection.onopen = null;
            this.connection.onclose = null;
            this.connection.onerror = null;
            this.connection.onmessage = null;
            if (this.connection.readyState === WebSocket.OPEN
                || this.connection.readyState === WebSocket.CONNECTING) {
                this.connection.close();
            }
        } catch (_e) { /* ignore */ }
        this.connection = null;
    }

    public connect(sessionId: string): void {
        if (this.currentState.state !== "new") {
            throw new Error(`connect() called from invalid state: ${this.currentState.state}`);
        }
        this.shouldReconnect = true;
        this.establish(sessionId);
    }

    public disconnect(): void {
        this.shouldReconnect = false;
        if (this.reconnectTimer !== null) {
            clearTimeout(this.reconnectTimer);
            this.reconnectTimer = null;
        }
        if (this.connection && this.connection.readyState === WebSocket.OPEN) {
            try {
                this.connection.send(JSON.stringify({
                    type: "disconnect",
                    payload: { reason: "client shutdown" },
                } satisfies C2SMessage));
            } catch (_) { /* ignore */ }
        }
        this.shutdownSocket(true);
        this.updateState({ state: "disconnected" });
    }

    private establish(sessionId: string): void {
        this.updateState({ state: "connecting" });
        let socket: WebSocket;
        try {
            socket = new WebSocket(this.targetAddress);
        } catch (err) {
            this.updateState({ state: "failed", reason: `WebSocket creation failed: ${String(err)}` });
            return;
        }
        this.connection = socket;
        this.stateUpdateStatistics.reset();

        socket.onopen = () => {
            this.updateState({ state: "handshaking" });
            this.sendRaw(JSON.stringify({
                type: "request-initialize",
                payload: { clientVersion: RADAR_PROTOCOL_VERSION },
            } satisfies HandshakeProtocolV2));
        };

        socket.onerror = () => {
            // onclose will fire immediately after with more details
        };

        socket.onclose = (ev) => {
            this.connection = null;
            if (!this.shouldReconnect) return;

            if (this.currentState.state === "failed"
                || this.currentState.state === "disconnected") {
                return;
            }

            // Exponential back-off: 250ms, 500ms, 1s, 2s, 4s, 8s (cap 15s)
            const backoff = Math.min(15000, 250 * Math.pow(2, this.reconnectAttempt));
            this.reconnectAttempt += 1;
            this.updateState({ state: "reconnecting", attempt: this.reconnectAttempt });
            log.debug(`[Aurora] Socket closed (code=${ev.code}), reconnecting in ${backoff}ms (attempt ${this.reconnectAttempt})`);

            this.reconnectTimer = setTimeout(() => {
                this.reconnectTimer = null;
                if (this.shouldReconnect) {
                    this.establish(sessionId);
                }
            }, backoff);
        };

        socket.onmessage = (event) => {
            try {
                const data = typeof event.data === "string" ? event.data : "";
                if (!data) return;

                if (this.currentState.state === "handshaking") {
                    const payload = JSON.parse(data) as HandshakeProtocolV2;
                    switch (payload.type) {
                        case "response-generic-failure":
                            this.updateState({ state: "failed", reason: payload.payload.message });
                            this.shutdownSocket(true);
                            break;
                        case "response-incompatible":
                            this.updateState({
                                state: "failed",
                                reason: `Protocol incompatible (supported versions: ${payload.payload.supportedVersions.join(", ")})`,
                            });
                            this.shutdownSocket(true);
                            break;
                        case "response-success":
                            log.debug(`[Aurora] Handshake OK — server version ${payload.payload.serverVersion}${payload.payload.serverName ? ` (${payload.payload.serverName})` : ""}`);
                            this.updateState({ state: "initializing" });
                            this.sendCommand("initialize-subscribe", { session_id: sessionId });
                            break;
                        default:
                            this.updateState({ state: "failed", reason: "Invalid handshake response" });
                            this.shutdownSocket(true);
                    }
                } else if (
                    this.currentState.state === "initializing"
                    || this.currentState.state === "connected"
                    || this.currentState.state === "reconnecting"
                ) {
                    const payload = JSON.parse(data) as S2CMessage;
                    const handler = this.commandHandler[payload.type];
                    if (handler) {
                        (handler as any)(payload.payload);
                    }
                }
            } catch (err) {
                log.warn("Failed to parse server message", err);
            }
        };
    }

    private sendRaw(data: string): void {
        if (this.connection && this.connection.readyState === WebSocket.OPEN) {
            try {
                this.connection.send(data);
            } catch (e) {
                this.pendingOutbound.push(data);
            }
        } else {
            this.pendingOutbound.push(data);
        }
    }

    public sendCommand<T extends C2SMessage["type"]>(
        command: T,
        payload: Extract<C2SMessage, { type: T }>["payload"],
    ): void {
        this.sendRaw(JSON.stringify({ type: command, payload }));
    }
}

/* small scoped logger that tolerates missing `log` global */
const nop = () => {};
const _globalLog: any = (globalThis as any).log;
const log = {
    debug: _globalLog?.debug ?? nop,
    warn: _globalLog?.warn ?? ((...a: any[]) => console.warn("[Aurora]", ...a)),
};

export const kDefaultRadarState: RadarState = {
    localControllerEntityId: null,
    playerPawns: [],
    worldName: "<empty>",
    c4Entities: [],
    plantedC4: null,
};
