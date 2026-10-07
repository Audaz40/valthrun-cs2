import { Box, Paper, Typography } from "@mui/material";
import * as colors from "@mui/material/colors";
import React from "react";
import { useAppSelector } from "../../../state";
import { PlantedC4State } from "../../../backend/definitions";
import IconC4 from "./icon_c4.svg";
import IconDefuse from "./icon_defuse.svg";

const StateBackground = (props: { state: PlantedC4State }) => {
    const { state } = props;

    let background, progress;
    switch (state.state) {
        case "active":
            if (state.defuser !== null) {
                progress = (state.defuser.timeTotal - state.defuser.timeRemaining) / state.defuser.timeTotal;
                background = "linear-gradient(90deg, #1a237e, #4fc3f7)";
            } else {
                progress = (state.timeTotal - state.timeDetonation) / state.timeTotal;
                background = "linear-gradient(90deg, #8b0000, #ff1744)";
            }
            break;

        case "defused":
            progress = 1;
            background = "linear-gradient(90deg, #1b5e20, #66bb6a)";
            break;

        case "detonated":
            progress = 1;
            background = "linear-gradient(90deg, #8b0000, #ff1744)";
            break;

        default:
            return null;
    }

    return (
        <Box
            sx={{
                position: "absolute",
                zIndex: 1,

                top: 0,
                left: 0,
                bottom: 0,

                background,
                boxShadow: state.state === "active" && state.defuser === null
                    ? "0 0 16px rgba(255, 23, 68, 0.55) inset"
                    : "none",
            }}
            width={`${(progress * 100).toFixed(0)}%`}
        />
    );
};

const formatTime = (time: number): string => {
    const minutes = Math.floor(time / 60);
    const seconds = Math.floor(time) - minutes * 60;
    const millis = time - Math.floor(time);
    if (minutes > 0) {
        return `${`${minutes}`.padStart(2, "0")}:${`${seconds}`.padStart(2, "0")}:${`${Math.round(millis * 100)}`.padStart(2, "0")}`;
    } else {
        return `${`${seconds}`.padStart(2, "0")}:${`${Math.round(millis * 100)}`.padStart(2, "0")}`;
    }
};

export default React.memo((props: { state: PlantedC4State }) => {
    const { state } = props;

    const bombDetailsOpacity = useAppSelector((state) => state.radarSettings.bombDetailsOpacity);

    let text, textColor;
    let Icon;
    let glow;
    switch (state.state) {
        case "active":
            if (state.defuser !== null) {
                text = formatTime(state.defuser.timeRemaining);
                Icon = IconDefuse;

                if (state.defuser.timeRemaining < state.timeDetonation) {
                    textColor = colors.green[400];
                } else {
                    textColor = "#ff5252";
                }
            } else {
                text = formatTime(state.timeDetonation);
                Icon = IconC4;
                textColor = "#ffffff";
                glow = "0 0 20px rgba(255, 23, 68, 0.6)";
            }
            break;

        case "defused":
            text = "defused";
            Icon = IconDefuse;
            textColor = colors.green[400];
            break;

        case "detonated":
            text = "detonated";
            Icon = IconC4;
            textColor = "#ffffff";
            glow = "0 0 20px rgba(255, 23, 68, 0.6)";
            break;
    }

    return (
        <Paper
            variant="outlined"
            sx={{
                width: "14em",
                height: "3.2em",
                position: "relative",
                overflow: "hidden",
                opacity: bombDetailsOpacity,
                borderColor: "rgba(255, 23, 68, 0.4) !important",
                boxShadow: glow || "0 0 12px rgba(139,0,0,0.3)",
                backdropFilter: "blur(8px)",
            }}
        >
            <Box
                sx={{
                    position: "absolute",
                    zIndex: 2,
                    top: 0,
                    left: 0,
                    right: 0,
                    bottom: 0,
                    display: "flex",
                    flexDirection: "row",
                    paddingLeft: 1.2,
                    paddingRight: 1.2,
                    "> *": {
                        alignSelf: "center",
                    },
                }}
            >
                <Icon width="2em" height="2em" fill={textColor as string} />
                <Typography
                    variant="h6"
                    sx={{
                        marginLeft: "auto",
                        marginRight: "auto",
                        color: textColor,
                        fontWeight: 600,
                        letterSpacing: 1,
                        fontFamily: "'Roboto Mono', monospace",
                    }}
                >
                    {text}
                </Typography>
            </Box>
            <StateBackground state={state} />
        </Paper>
    );
});
