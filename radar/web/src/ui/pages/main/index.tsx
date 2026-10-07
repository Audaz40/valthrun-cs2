import {
    Box,
    Button,
    TextField,
    Typography,
    IconButton,
    Tooltip,
    Divider,
} from "@mui/material";
import { Visibility as IconEye, ContentCopy as IconCopy, Radar as IconRadar, Security as IconShield, Speed as IconSpeed, Groups as IconGroups } from "@mui/icons-material";
import * as React from "react";
import { useNavigate } from "react-router-dom";
import LogoAurora from "../../../assets/aurora-logo.svg";

const FeatureItem = React.memo((props: { icon: React.ReactNode; title: string; description: string }) => (
    <Box
        sx={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            gap: 1,
            p: 2.5,
            borderRadius: 2,
            border: "1px solid rgba(255, 23, 68, 0.15)",
            background: "linear-gradient(180deg, rgba(40, 10, 10, 0.6), rgba(20, 5, 5, 0.8))",
            backdropFilter: "blur(8px)",
            transition: "all .25s ease",
            "&:hover": {
                borderColor: "rgba(255, 23, 68, 0.4)",
                boxShadow: "0 0 24px rgba(255, 23, 68, 0.15)",
                transform: "translateY(-2px)",
            },
            width: { xs: "100%", sm: 180 },
        }}
    >
        <Box sx={{
            color: "#ff1744",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            width: 48,
            height: 48,
            borderRadius: "50%",
            background: "rgba(255, 23, 68, 0.08)",
            mb: 1,
        }}>
            {props.icon}
        </Box>
        <Typography variant="subtitle1" sx={{ fontWeight: 600, color: "#ffcdd2", textAlign: "center" }}>
            {props.title}
        </Typography>
        <Typography variant="body2" sx={{ color: "#b08080", textAlign: "center", fontSize: "0.82rem", lineHeight: 1.5 }}>
            {props.description}
        </Typography>
    </Box>
));

export default React.memo(() => {
    const navigate = useNavigate();
    const [sessionId, setSessionId] = React.useState("");
    const [copied, setCopied] = React.useState(false);

    const handleConnect = React.useCallback(() => {
        const id = sessionId.trim();
        if (id) {
            navigate(`/session/${encodeURIComponent(id)}`);
        }
    }, [sessionId, navigate]);

    const handleKeyDown = React.useCallback((e: React.KeyboardEvent) => {
        if (e.key === "Enter") handleConnect();
    }, [handleConnect]);

    const handlePasteFromClipboard = React.useCallback(async () => {
        try {
            const text = await navigator.clipboard.readText();
            if (text) setSessionId(text.trim());
        } catch (err) {
            /* clipboard permission denied */
        }
    }, []);

    const handleCopyDemo = React.useCallback(() => {
        setSessionId("aurora-demo");
        navigator.clipboard?.writeText("aurora-demo");
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
    }, []);

    return (
        <Box
            sx={{
                height: "100%",
                width: "100%",
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
                justifyContent: "center",
                px: 3,
                overflowY: "auto",
                py: 4,
            }}
        >
            {/* Header / Brand */}
            <Box
                sx={{
                    display: "flex",
                    flexDirection: "column",
                    alignItems: "center",
                    mb: 4,
                }}
            >
                <Box
                    component={LogoAurora}
                    sx={{
                        width: 110,
                        height: 110,
                        filter: "drop-shadow(0 0 24px rgba(255, 23, 68, 0.45))",
                        mb: 2,
                        animation: "aurora-float 4s ease-in-out infinite",
                        "@keyframes aurora-float": {
                            "0%, 100%": { transform: "translateY(0)" },
                            "50%":      { transform: "translateY(-6px)" },
                        },
                    }}
                />
                <Typography
                    variant="h2"
                    sx={{
                        fontWeight: 800,
                        letterSpacing: 6,
                        background: "linear-gradient(135deg, #ff5252 0%, #ff1744 50%, #8b0000 100%)",
                        backgroundClip: "text",
                        WebkitBackgroundClip: "text",
                        WebkitTextFillColor: "transparent",
                        textTransform: "uppercase",
                        fontSize: { xs: "2.5rem", sm: "3.4rem", md: "4rem" },
                        textShadow: "0 0 40px rgba(255, 23, 68, 0.3)",
                    }}
                >
                    AURORA
                </Typography>
                <Typography
                    variant="subtitle1"
                    sx={{
                        color: "#b08080",
                        letterSpacing: 3,
                        mt: 0.5,
                        textTransform: "uppercase",
                        fontSize: "0.8rem",
                        fontWeight: 500,
                    }}
                >
                    Tactical Radar System
                </Typography>
                <Divider sx={{ width: 80, mt: 2, borderColor: "rgba(255, 23, 68, 0.3)", borderBottomWidth: 2 }} />
            </Box>

            {/* Session connect card */}
            <Box
                sx={{
                    display: "flex",
                    flexDirection: "column",
                    gap: 2,
                    width: "100%",
                    maxWidth: "28em",
                    p: 3.5,
                    borderRadius: 2,
                    border: "1px solid rgba(255, 23, 68, 0.2)",
                    background: "linear-gradient(180deg, rgba(40, 10, 10, 0.75), rgba(14, 3, 3, 0.9))",
                    backdropFilter: "blur(16px)",
                    boxShadow: "0 0 60px rgba(139, 0, 0, 0.3), inset 0 1px 0 rgba(255, 23, 68, 0.08)",
                    mb: 4,
                }}
            >
                <Typography variant="h6" sx={{ color: "#ffcdd2", fontWeight: 600, mb: 0.5 }}>
                    Connect to Session
                </Typography>
                <Typography variant="body2" sx={{ color: "#b08080", mb: 1, fontSize: "0.85rem" }}>
                    Enter the session ID shared by your broadcaster to view the live tactical radar feed.
                </Typography>

                <Box sx={{ display: "flex", gap: 1, alignItems: "stretch" }}>
                    <TextField
                        fullWidth
                        value={sessionId}
                        onChange={(event) => setSessionId(event.target.value)}
                        onKeyDown={handleKeyDown}
                        placeholder={"Session ID"}
                        size={"small"}
                        InputProps={{
                            sx: { fontFamily: "'Roboto Mono', monospace", letterSpacing: 1 },
                        }}
                    />
                    <Tooltip title="Paste from clipboard">
                        <IconButton
                            onClick={handlePasteFromClipboard}
                            sx={{
                                border: "1px solid rgba(255, 23, 68, 0.2)",
                                borderRadius: 1,
                                color: "#ef5350",
                            }}
                        >
                            <IconEye />
                        </IconButton>
                    </Tooltip>
                </Box>

                <Button
                    variant="contained"
                    onClick={handleConnect}
                    disabled={sessionId.trim() === ""}
                    sx={{
                        py: 1.2,
                        fontSize: "0.95rem",
                    }}
                    startIcon={<IconRadar />}
                >
                    Enter Radar
                </Button>

                <Box sx={{ display: "flex", alignItems: "center", gap: 1, mt: 0.5 }}>
                    <Divider sx={{ flex: 1, borderColor: "rgba(255, 23, 68, 0.1)" }} />
                    <Typography variant="caption" sx={{ color: "#704040", textTransform: "uppercase", letterSpacing: 1, fontSize: "0.7rem" }}>
                        Quick Start
                    </Typography>
                    <Divider sx={{ flex: 1, borderColor: "rgba(255, 23, 68, 0.1)" }} />
                </Box>

                <Button
                    variant="outlined"
                    onClick={handleCopyDemo}
                    size="small"
                    startIcon={<IconCopy />}
                    sx={{ py: 0.7, fontSize: "0.8rem" }}
                >
                    {copied ? "Copied!" : "Try demo session: aurora-demo"}
                </Button>
            </Box>

            {/* Feature highlights */}
            <Box
                sx={{
                    display: "flex",
                    flexDirection: { xs: "column", sm: "row" },
                    gap: 2,
                    flexWrap: "wrap",
                    justifyContent: "center",
                    maxWidth: "60em",
                }}
            >
                <FeatureItem
                    icon={<IconRadar fontSize="medium" />}
                    title="Live Radar"
                    description="Real-time player positions across all maps with sub-100ms updates."
                />
                <FeatureItem
                    icon={<IconShield fontSize="medium" />}
                    title="Read-Only"
                    description="External, non-invasive memory reads. Zero writes to process memory."
                />
                <FeatureItem
                    icon={<IconSpeed fontSize="medium" />}
                    title="Stream-Safe"
                    description="The overlay is hidden from screen capture by default."
                />
                <FeatureItem
                    icon={<IconGroups fontSize="medium" />}
                    title="Shareable"
                    description="Broadcast your match to teammates via a single session URL."
                />
            </Box>

            {/* Footer */}
            <Box sx={{ mt: 5, textAlign: "center" }}>
                <Typography variant="caption" sx={{ color: "#704040", letterSpacing: 2, fontSize: "0.7rem" }}>
                    AURORA · PRECISION VISION · RED THEME EDITION
                </Typography>
            </Box>
        </Box>
    );
});
