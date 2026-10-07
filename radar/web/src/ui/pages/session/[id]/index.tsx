import { Settings as IconSettings, Logout as IconLogout } from "@mui/icons-material";
import { Alert, Box, CircularProgress, IconButton, Typography, Tooltip } from "@mui/material";
import * as React from "react";
import { useNavigate, useParams } from "react-router-dom";
import { kDefaultRadarState } from "../../../../backend/connection";
import { useAppDispatch } from "../../../../state";
import { updateRadarSettings } from "../../../../state/radar-settings";
import { SubscriberClientProvider, useSubscriberClient } from "../../../components/connection";
import ModalSettings from "./modal-settings";
import { RadarRenderer } from "./radar";
import { useDocumentFocusState } from "../../../components/container/document-focus-state";
import LogoAurora from "../../../../assets/aurora-logo.svg";

const kServerUrl: string | null = process.env.SERVER_URL;
const getEndpointUrl = () => {
    if (typeof kServerUrl === "string") {
        return kServerUrl;
    }

    const urlSearch = new URLSearchParams(location.search ?? "");
    if (urlSearch.has("endpoint")) {
        return urlSearch.get("endpoint");
    }

    const parts = [];
    if (location.protocol === "https:") {
        parts.push("wss://");
    } else {
        parts.push("ws://");
    }
    parts.push(location.hostname);
    if (location.port) {
        parts.push(`:${location.port}`);
    }
    parts.push("/subscribe");

    return parts.join("");
}

export default React.memo(() => {
    const targetUrl = React.useMemo(getEndpointUrl, []);

    return (
        <Box
            sx={{
                height: "100%",
                width: "100%",
                display: "flex",
                flexDirection: "column",
                justifyContent: "center",
                position: "relative",
            }}
        >
            <SubscriberClientProvider address={targetUrl}>
                <ClientStateNew />
                <ClientStateConnecting />
                <ClientStateFailed />
                <ClientStateConnected />
                <ClientStateDisconnected />
                <ModalSettings />
            </SubscriberClientProvider>

            {/* Aurora branding watermark */}
            <Box sx={{ position: "absolute", top: 12, left: 16, display: "flex", alignItems: "center", gap: 1.2, zIndex: 1, pointerEvents: "none", opacity: 0.85 }}>
                <Box component={LogoAurora} sx={{ width: 28, height: 28, filter: "drop-shadow(0 0 8px rgba(255,23,68,0.6))" }} />
                <Typography sx={{
                    fontWeight: 700,
                    letterSpacing: 3,
                    fontSize: "0.9rem",
                    color: "#ff5252",
                    textTransform: "uppercase",
                    textShadow: "0 0 10px rgba(255,23,68,0.5)",
                }}>
                    Aurora
                </Typography>
            </Box>

            <Box sx={{ position: "absolute", top: 8, right: 12, display: "flex", gap: 0.5, zIndex: 2 }}>
                <ButtonBackToMenu />
                <ButtonToggleSettings />
            </Box>
        </Box>
    );
});

const ButtonBackToMenu = React.memo(() => {
    const navigate = useNavigate();
    const hasFocus = useDocumentFocusState();
    return (
        <Tooltip title="Back to menu">
            <IconButton
                onClick={() => navigate("/")}
                sx={{
                    opacity: hasFocus ? 1 : 0.25,
                    transition: ".15s ease-in-out",
                    border: "1px solid rgba(255, 23, 68, 0.15)",
                    borderRadius: 1.5,
                }}
            >
                <IconLogout />
            </IconButton>
        </Tooltip>
    );
});

const ButtonToggleSettings = React.memo(() => {
    const dispatch = useAppDispatch();
    const hasFocus = useDocumentFocusState();
    return (
        <Tooltip title="Settings">
            <IconButton
                onClick={() => dispatch(updateRadarSettings({ dialogOpen: true }))}
                sx={{
                    zIndex: 1,
                    opacity: hasFocus ? 1 : 0.25,
                    transition: ".15s ease-in-out",
                    border: "1px solid rgba(255, 23, 68, 0.15)",
                    borderRadius: 1.5,
                }}
            >
                <IconSettings />
            </IconButton>
        </Tooltip>
    );
});

const useSubscriberClientState = () => {
    const client = useSubscriberClient();
    const [state, setState] = React.useState(() => client.getState());
    React.useEffect(() => client.events.on("state_changed", (newState) => setState(newState)), [client]);
    return state;
};

const ClientStateNew = React.memo(() => {
    const client = useSubscriberClient();
    const { state } = useSubscriberClientState();
    const { sessionId } = useParams() as any;

    React.useEffect(() => {
        if (state !== "new") {
            return;
        }

        client.connect(sessionId);
    }, [client, state]);

    if (state !== "new") {
        return null;
    }

    if (!sessionId) {
        return <Alert severity={"error"}>Missing session id in URL</Alert>;
    }

    return null;
});

const ClientStateConnecting = React.memo(() => {
    const { state } = useSubscriberClientState();
    if (state !== "connecting") {
        return;
    }

    return (
        <Box sx={{
            alignSelf: "center",
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            gap: 2,
            p: 4,
        }}>
            <CircularProgress size={48} sx={{ color: "#ff1744" }} />
            <Typography sx={{ color: "#ffcdd2", letterSpacing: 2, textTransform: "uppercase", fontSize: "0.85rem" }}>
                Establishing Link
            </Typography>
        </Box>
    );
});

const ClientStateFailed = React.memo(() => {
    const state = useSubscriberClientState();
    if (state.state !== "failed") {
        return;
    }

    return (
        <Box sx={{
            alignSelf: "center",
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            gap: 1.5,
            p: 4,
            border: "1px solid rgba(255, 23, 68, 0.3)",
            borderRadius: 2,
            background: "rgba(40, 10, 10, 0.8)",
            backdropFilter: "blur(8px)",
            maxWidth: "28em",
        }}>
            <Typography variant="h6" sx={{ color: "#ff5252", letterSpacing: 2, textTransform: "uppercase" }}>
                Connection Failed
            </Typography>
            <Typography sx={{ color: "#b08080", textAlign: "center", fontSize: "0.9rem" }}>
                {state.reason}
            </Typography>
        </Box>
    );
});

const ClientStateDisconnected = React.memo(() => {
    const state = useSubscriberClientState();
    const navigate = useNavigate();
    if (state.state !== "disconnected") {
        return;
    }

    return (
        <Box sx={{
            alignSelf: "center",
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            gap: 2,
            p: 4,
            border: "1px solid rgba(255, 23, 68, 0.25)",
            borderRadius: 2,
            background: "rgba(40, 10, 10, 0.8)",
            backdropFilter: "blur(8px)",
        }}>
            <Typography variant="h6" sx={{ color: "#ff5252", letterSpacing: 2, textTransform: "uppercase" }}>
                Session Closed
            </Typography>
            <Typography sx={{ color: "#b08080", fontSize: "0.9rem" }}>
                The broadcaster has ended the session.
            </Typography>
            <Box
                component="button"
                onClick={() => navigate("/")}
                sx={{
                    mt: 1,
                    px: 3,
                    py: 1,
                    background: "linear-gradient(135deg, #8b0000, #d32f2f)",
                    border: "none",
                    borderRadius: 1,
                    color: "#fff",
                    fontWeight: 600,
                    letterSpacing: 1,
                    cursor: "pointer",
                    fontSize: "0.85rem",
                    textTransform: "uppercase",
                    boxShadow: "0 0 20px rgba(255, 23, 68, 0.25)",
                    "&:hover": {
                        background: "linear-gradient(135deg, #d32f2f, #ff1744)",
                        boxShadow: "0 0 28px rgba(255, 23, 68, 0.5)",
                    },
                }}
            >
                Return to Menu
            </Box>
        </Box>
    );
});

const ClientStateConnected = React.memo(() => {
    const client = useSubscriberClient();
    const state = useSubscriberClientState();
    const [viewerCount, setViewerCount] = React.useState(0);

    React.useEffect(() => {
        const onViewers = (v: number) => setViewerCount(v);
        client.events.on("view.count", onViewers);
        return () => {
            client.events.off("view.count", onViewers);
        };
    }, [client]);

    if (state.state !== "connected") {
        return;
    }

    return (
        <Box
            sx={{
                alignSelf: "center",
                height: "100%",
                width: "100%",
                display: "flex",
                flexDirection: "column",
                justifyContent: "center",
            }}
        >
            <RadarRenderer viewerCount={viewerCount} />
        </Box>
    );
});
