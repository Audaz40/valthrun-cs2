import * as React from "react";
import "@fontsource/roboto/300.css";
import "@fontsource/roboto/400.css";
import "@fontsource/roboto/500.css";
import "@fontsource/roboto/700.css";
import "./app.scss";
import { ThemeProvider } from "@emotion/react";
import { Box, createTheme, CssBaseline } from "@mui/material";
import { Provider as StateProvider } from "react-redux";
import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { RecoilRoot } from "recoil";
import { appStore } from "../state";
import PageMain from "./pages/main";
import PageSession from "./pages/session/[id]";
import { QueryClient, QueryClientProvider } from "react-query";
import { DocumentFocusStateProvider } from "./components/container/document-focus-state";

// Aurora dark-red theme
const theme = createTheme({
    palette: {
        mode: "dark",
        primary: {
            main: "#ff1744",
            light: "#ff5252",
            dark: "#8b0000",
            contrastText: "#ffffff",
        },
        secondary: {
            main: "#d32f2f",
            light: "#ef5350",
            dark: "#6a0000",
            contrastText: "#ffffff",
        },
        error: {
            main: "#ff5252",
            dark: "#c62828",
        },
        warning: {
            main: "#ff6e40",
        },
        success: {
            main: "#4caf50",
        },
        background: {
            default: "#0a0000",
            paper: "rgba(40, 10, 10, 0.85)",
        },
        text: {
            primary: "#f5e0e0",
            secondary: "#b08080",
            disabled: "#704040",
        },
        divider: "rgba(255, 23, 68, 0.2)",
    },
    typography: {
        fontFamily: "'Roboto', sans-serif",
        h1: { fontWeight: 700, letterSpacing: 1 },
        h2: { fontWeight: 700, letterSpacing: 0.8 },
        h3: { fontWeight: 700, letterSpacing: 0.6 },
        h4: { fontWeight: 600, letterSpacing: 0.5 },
        h5: { fontWeight: 600, letterSpacing: 0.5 },
        h6: { fontWeight: 600 },
        button: {
            fontWeight: 600,
            letterSpacing: 0.5,
        },
    },
    shape: {
        borderRadius: 6,
    },
    components: {
        MuiCssBaseline: {
            styleOverrides: {
                body: {
                    scrollbarColor: "#8b0000 #140505",
                },
            },
        },
    },
});

export const queryClient = new QueryClient({
    defaultOptions: {
        queries: {
            refetchOnWindowFocus: false,
            staleTime: 60000,
        },
    },
});

export const App = React.memo(() => {
    return (
        <React.Fragment>
            <StateProvider store={appStore}>
                <RecoilRoot>
                    <ThemeProvider theme={theme}>
                        <CssBaseline />
                        <DocumentFocusStateProvider>
                            <QueryClientProvider client={queryClient}>
                                <BrowserRouter>
                                    <Box
                                        sx={{
                                            height: "100%",
                                            width: "100%",
                                            position: "relative",
                                            zIndex: 1,
                                        }}
                                    >
                                        <Routes>
                                            <Route path="/" element={<PageMain />} />
                                            <Route path="/session/:sessionId" element={<PageSession />} />
                                            <Route path={"*"} element={<Navigate to={"/"} />} />
                                        </Routes>
                                    </Box>
                                </BrowserRouter>
                            </QueryClientProvider>
                        </DocumentFocusStateProvider>
                    </ThemeProvider>
                </RecoilRoot>
            </StateProvider>
        </React.Fragment>
    );
});
