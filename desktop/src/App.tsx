import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { useEffect, useRef, useState } from "react";

type TerminalMode = "native" | "translate";
type TerminalAction = "execute" | "confirm" | "block" | "unknown_no_execute";
type ConfirmationResolutionAction = "execute" | "cancelled";

type SessionStartResponse = {
  sessionId: number;
  currentOs: string;
};

type SubmitTerminalLineResponse = {
  originalCommand: string;
  detectedSource: string;
  currentOs: string;
  matchedIntent: string | null;
  translatedCommand: string | null;
  riskLevel: string;
  action: TerminalAction;
  riskReason: string | null;
  message: string | null;
  confirmationPrompt: string | null;
  stdout: string;
  stderr: string;
  exitStatus: number | null;
  currentDir: string;
  clearDisplay: boolean;
};

type ConfirmationResolutionResponse = {
  action: ConfirmationResolutionAction;
  translatedCommand: string | null;
  message: string;
  stdout: string;
  stderr: string;
  exitStatus: number | null;
  currentDir: string;
  clearDisplay: boolean;
};

type PtyOutputEvent = {
  sessionId: number;
  data: string;
};

type PtyExitEvent = {
  sessionId: number;
};

const MODE_RULE = "------------------------------------------------";
const SECTION_RULE = "-----------------------------------------------------";
const TRANSLATE_PROMPT = "cli4all-translate> ";
const CONFIRM_PROMPT = "Execute this command? [y/N]";
const SCROLLBACK_LINES = 20000;
const BOTTOM_SCROLL_THRESHOLD = 2;
const BACKGROUND_PTY_BUFFER_LIMIT = 200_000;

export default function App() {
  const terminalContainerRef = useRef<HTMLDivElement | null>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const sessionIdRef = useRef<number | null>(null);
  const modeRef = useRef<TerminalMode>("native");
  const translateBufferRef = useRef("");
  const confirmationBufferRef = useRef("");
  const localPromptVisibleRef = useRef(false);
  const awaitingConfirmationRef = useRef(false);
  const hiddenPtyOutputRef = useRef("");
  const destroyedRef = useRef(false);

  const [mode, setMode] = useState<TerminalMode>("native");
  const [currentOs, setCurrentOs] = useState("Starting...");

  useEffect(() => {
    modeRef.current = mode;
  }, [mode]);

  useEffect(() => {
    const container = terminalContainerRef.current;
    if (!container) {
      return;
    }

    destroyedRef.current = false;

    const terminal = new Terminal({
      cursorBlink: true,
      scrollback: SCROLLBACK_LINES,
      convertEol: true,
      fontFamily: '"SFMono-Regular", "IBM Plex Mono", "Menlo", monospace',
      fontSize: 14,
      lineHeight: 1.35,
      theme: {
        background: "#07111c",
        foreground: "#dce7f3",
        cursor: "#f8d66d",
        black: "#07111c",
        blue: "#62b1ff",
        brightBlack: "#486174",
        brightBlue: "#9dd1ff",
        brightCyan: "#97f1e3",
        brightGreen: "#c0e77d",
        brightMagenta: "#f3a4b8",
        brightRed: "#ff8f7a",
        brightWhite: "#f5fbff",
        brightYellow: "#ffd479",
        cyan: "#6cd6c3",
        green: "#91d45a",
        magenta: "#df87a6",
        red: "#ff6b57",
        white: "#dce7f3",
        yellow: "#f3c65f",
      },
    });

    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(container);
    fitAddon.fit();

    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;
    terminal.focus();

    const isViewportNearBottom = () => {
      const buffer = terminal.buffer.active;
      return buffer.baseY - buffer.viewportY <= BOTTOM_SCROLL_THRESHOLD;
    };

    const writeTerminal = (
      data: string,
      scrollBehavior: "always" | "if-bottom" | "never" = "never",
    ) => {
      const shouldScroll =
        scrollBehavior === "always" ||
        (scrollBehavior === "if-bottom" && isViewportNearBottom());

      terminal.write(data, () => {
        if (shouldScroll) {
          terminal.scrollToBottom();
        }
      });
    };

    const writeTerminalLine = (
      line: string,
      scrollBehavior: "always" | "if-bottom" | "never" = "never",
    ) => {
      writeTerminal(`${line}\r\n`, scrollBehavior);
    };

    const writeStream = (data: string) => {
      writeTerminal(data, "if-bottom");
    };

    const printLines = (lines: string[]) => {
      lines.forEach((line) => writeTerminalLine(line, "always"));
    };

    const printNotice = (title: string, lines: string[]) => {
      printLines([
        `---------------- ${title} ----------------`,
        ...lines,
        SECTION_RULE,
      ]);
    };

    const printTranslation = (response: SubmitTerminalLineResponse) => {
      printLines([
        "---------------- CLI4ALL Translation ----------------",
        `Original command:   ${response.originalCommand}`,
        `Detected source:    ${response.detectedSource}`,
        `Current OS:         ${response.currentOs}`,
        `Matched intent:     ${response.matchedIntent ?? "unknown"}`,
        `Translated command: ${response.translatedCommand ?? "unavailable"}`,
        `Risk level:         ${response.riskLevel}`,
        SECTION_RULE,
      ]);
    };

    const printExecutionSections = (
      stdout: string,
      stderr: string,
      exitStatus: number | null,
    ) => {
      if (stdout.length > 0 || stderr.length > 0) {
        printLines(["---------------- Command Output ----------------"]);

        if (stdout.length > 0) {
          writeTerminalLine("[stdout]", "always");
          writeTerminal(ensureTrailingNewline(stdout), "always");
        }

        if (stderr.length > 0) {
          writeTerminalLine("[stderr]", "always");
          writeTerminal(ensureTrailingNewline(stderr), "always");
        }

        printLines(["------------------------------------------------"]);
      }

      printLines([
        "---------------- Execution Result ----------------",
        `Exit status: ${exitStatus ?? "unavailable"}`,
        "--------------------------------------------------",
      ]);
    };

    const resetLocalState = () => {
      translateBufferRef.current = "";
      confirmationBufferRef.current = "";
      localPromptVisibleRef.current = false;
      awaitingConfirmationRef.current = false;
    };

    const showTranslatePrompt = (prependNewline: boolean) => {
      if (modeRef.current !== "translate") {
        return;
      }
      if (awaitingConfirmationRef.current || localPromptVisibleRef.current) {
        return;
      }

      if (prependNewline) {
        writeTerminal("\r\n", "always");
      }

      writeTerminal(TRANSLATE_PROMPT, "always");
      localPromptVisibleRef.current = true;
    };

    const syncPtySize = async () => {
      const currentTerminal = terminalRef.current;
      const currentFitAddon = fitAddonRef.current;
      if (!currentTerminal || !currentFitAddon) {
        return;
      }

      currentFitAddon.fit();
      if (sessionIdRef.current === null) {
        return;
      }

      try {
        await invoke("resize_pty", {
          cols: currentTerminal.cols,
          rows: currentTerminal.rows,
        });
      } catch {
        // Ignore resize races during session restarts.
      }
    };

    const printTranslateExecution = (
      response: Pick<
        SubmitTerminalLineResponse,
        "stdout" | "stderr" | "exitStatus" | "clearDisplay"
      >,
    ) => {
      printExecutionSections(response.stdout, response.stderr, response.exitStatus);
      showTranslatePrompt(false);
    };

    const cancelPendingConfirmation = async (printCancellation: boolean) => {
      if (!awaitingConfirmationRef.current) {
        return;
      }

      awaitingConfirmationRef.current = false;
      confirmationBufferRef.current = "";
      localPromptVisibleRef.current = false;

      try {
        await invoke<ConfirmationResolutionResponse>("resolve_confirmation", {
          confirmed: false,
        });
      } catch (error) {
        printNotice("CLI4ALL Notice", [`Backend error: ${String(error)}`]);
        return;
      }

      if (printCancellation) {
        printNotice("CLI4ALL Notice", ["Execution cancelled."]);
      }
    };

    const handleSubmitResponse = (response: SubmitTerminalLineResponse) => {
      if (response.clearDisplay) {
        terminal.clear();
        terminal.scrollToBottom();
      }

      printTranslation(response);

      switch (response.action) {
        case "execute":
          printTranslateExecution(response);
          break;
        case "confirm":
          awaitingConfirmationRef.current = true;
          confirmationBufferRef.current = "";
          writeTerminalLine(response.confirmationPrompt ?? CONFIRM_PROMPT, "always");
          break;
        case "block":
          printNotice("CLI4ALL Safety", [
            response.message ?? "Destructive command blocked by CLI4ALL.",
            response.riskReason
              ? `Reason: ${response.riskReason}`
              : "Reason: blocked by safety policy.",
          ]);
          showTranslatePrompt(false);
          break;
        case "unknown_no_execute":
          printNotice("CLI4ALL Notice", [
            response.message ??
              "Unknown command mapping. CLI4ALL will not execute this command automatically in safe mode.",
          ]);
          showTranslatePrompt(false);
          break;
      }
    };

    const submitTranslateLine = async () => {
      const input = translateBufferRef.current;
      translateBufferRef.current = "";
      localPromptVisibleRef.current = false;
      writeTerminal("\r\n", "always");

      if (input.trim().length === 0) {
        showTranslatePrompt(false);
        return;
      }

      try {
        const response = await invoke<SubmitTerminalLineResponse>(
          "submit_terminal_line",
          {
            input,
          },
        );
        handleSubmitResponse(response);
      } catch (error) {
        printNotice("CLI4ALL Notice", [`Backend error: ${String(error)}`]);
        showTranslatePrompt(false);
      }
    };

    const resolveConfirmation = async () => {
      const approved = matchesYes(confirmationBufferRef.current);
      confirmationBufferRef.current = "";
      awaitingConfirmationRef.current = false;
      localPromptVisibleRef.current = false;
      writeTerminal("\r\n", "always");

      try {
        const response = await invoke<ConfirmationResolutionResponse>(
          "resolve_confirmation",
          {
            confirmed: approved,
          },
        );

        if (response.action === "cancelled") {
          printNotice("CLI4ALL Notice", [response.message]);
          showTranslatePrompt(false);
          return;
        }

        if (response.clearDisplay) {
          terminal.clear();
          terminal.scrollToBottom();
        }

        printExecutionSections(
          response.stdout,
          response.stderr,
          response.exitStatus,
        );
        showTranslatePrompt(false);
      } catch (error) {
        printNotice("CLI4ALL Notice", [`Backend error: ${String(error)}`]);
        showTranslatePrompt(false);
      }
    };

    const startSession = async () => {
      resetLocalState();
      hiddenPtyOutputRef.current = "";
      sessionIdRef.current = null;
      terminal.reset();
      terminal.scrollToBottom();
      terminal.focus();

      try {
        const response = await invoke<SessionStartResponse>("start_pty_session", {
          cols: terminal.cols,
          rows: terminal.rows,
        });
        sessionIdRef.current = response.sessionId;
        setCurrentOs(response.currentOs);
        await syncPtySize();

        if (modeRef.current === "translate") {
          showTranslatePrompt(false);
        }
      } catch (error) {
        printNotice("CLI4ALL Notice", [
          `Failed to start PTY session: ${String(error)}`,
        ]);
      }
    };

    const handleNativeInput = (data: string) => {
      terminal.scrollToBottom();
      void invoke("write_to_pty", { input: data }).catch((error) => {
        printNotice("CLI4ALL Notice", [`Backend error: ${String(error)}`]);
      });
    };

    const handleTranslateInput = (data: string) => {
      if (awaitingConfirmationRef.current) {
        switch (data) {
          case "\r":
            void resolveConfirmation();
            return;
          case "\u007F":
            if (confirmationBufferRef.current.length > 0) {
              terminal.scrollToBottom();
              confirmationBufferRef.current =
                confirmationBufferRef.current.slice(0, -1);
              writeTerminal("\b \b", "always");
            }
            return;
          case "\u0003":
            writeTerminal("^C\r\n", "always");
            void cancelPendingConfirmation(true).then(() => {
              showTranslatePrompt(false);
            });
            return;
          default:
            if (isPrintableInput(data)) {
              terminal.scrollToBottom();
              confirmationBufferRef.current += data;
              writeTerminal(data, "always");
            }
            return;
        }
      }

      switch (data) {
        case "\r":
          void submitTranslateLine();
          return;
        case "\u007F":
          if (translateBufferRef.current.length > 0) {
            terminal.scrollToBottom();
            translateBufferRef.current = translateBufferRef.current.slice(0, -1);
            writeTerminal("\b \b", "always");
          }
          return;
        case "\u0003":
          if (translateBufferRef.current.length > 0 || localPromptVisibleRef.current) {
            translateBufferRef.current = "";
            localPromptVisibleRef.current = false;
            writeTerminal("^C\r\n", "always");
            showTranslatePrompt(false);
          } else {
            handleNativeInput("\u0003");
          }
          return;
        default:
          if (!isPrintableInput(data)) {
            return;
          }

          terminal.scrollToBottom();
          if (!localPromptVisibleRef.current) {
            showTranslatePrompt(true);
          }

          translateBufferRef.current += data;
          writeTerminal(data, "always");
      }
    };

    const handleData = (data: string) => {
      if (modeRef.current === "native") {
        handleNativeInput(data);
        return;
      }

      handleTranslateInput(data);
    };

    const resizeObserver = new ResizeObserver(() => {
      void syncPtySize();
    });
    resizeObserver.observe(container);

    const handleWindowResize = () => {
      void syncPtySize();
    };

    window.addEventListener("resize", handleWindowResize);

    const dataDisposable = terminal.onData(handleData);

    let unlistenOutput: (() => void) | undefined;
    let unlistenExit: (() => void) | undefined;

    void listen<PtyOutputEvent>("pty-output", (event) => {
      if (destroyedRef.current) {
        return;
      }
      if (
        sessionIdRef.current !== null &&
        event.payload.sessionId !== sessionIdRef.current
      ) {
        return;
      }

      if (modeRef.current === "translate") {
        hiddenPtyOutputRef.current = appendBufferedPtyOutput(
          hiddenPtyOutputRef.current,
          event.payload.data,
        );
        return;
      }

      writeStream(event.payload.data);
    }).then((unlisten) => {
      unlistenOutput = unlisten;
    });

    void listen<PtyExitEvent>("pty-exit", (event) => {
      if (destroyedRef.current) {
        return;
      }
      if (
        sessionIdRef.current !== null &&
        event.payload.sessionId !== sessionIdRef.current
      ) {
        return;
      }
      sessionIdRef.current = null;
      hiddenPtyOutputRef.current = "";
      resetLocalState();
      printNotice("CLI4ALL Notice", [
        "PTY session ended. Use New Session to start another terminal.",
      ]);
    }).then((unlisten) => {
      unlistenExit = unlisten;
    });

    writeTerminalLine("CLI4ALL PTY terminal", "always");
    void startSession();

    return () => {
      destroyedRef.current = true;
      dataDisposable.dispose();
      resizeObserver.disconnect();
      window.removeEventListener("resize", handleWindowResize);
      void invoke("stop_pty_session").catch(() => undefined);
      unlistenOutput?.();
      unlistenExit?.();
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
      sessionIdRef.current = null;
    };
  }, []);

  const toggleMode = async () => {
    const terminal = terminalRef.current;
    if (!terminal) {
      return;
    }

    await cancelConfirmationFromToolbar();

    translateBufferRef.current = "";
    confirmationBufferRef.current = "";
    localPromptVisibleRef.current = false;
    awaitingConfirmationRef.current = false;

    const nextMode: TerminalMode =
      modeRef.current === "native" ? "translate" : "native";
    modeRef.current = nextMode;
    setMode(nextMode);

    terminal.scrollToBottom();
    writeTerminalAndScroll(terminal, "\r\n");
    printToolbarModeNotice(terminal, nextMode);

    if (nextMode === "translate") {
      terminal.focus();
      writeTerminalAndScroll(terminal, TRANSLATE_PROMPT);
      localPromptVisibleRef.current = true;
      return;
    }

    const bufferedOutput = hiddenPtyOutputRef.current;
    hiddenPtyOutputRef.current = "";
    if (bufferedOutput.length > 0) {
      writeTerminalAndScroll(terminal, bufferedOutput);
    } else if (sessionIdRef.current !== null) {
      void invoke("write_to_pty", { input: "\n" }).catch(() => undefined);
    }
  };

  const startNewSession = async () => {
    const terminal = terminalRef.current;
    const fitAddon = fitAddonRef.current;
    if (!terminal || !fitAddon) {
      return;
    }

    await cancelConfirmationFromToolbar();

    translateBufferRef.current = "";
    confirmationBufferRef.current = "";
    localPromptVisibleRef.current = false;
    awaitingConfirmationRef.current = false;
    hiddenPtyOutputRef.current = "";

    fitAddon.fit();
    terminal.reset();
    terminal.scrollToBottom();
    terminal.focus();

    try {
      const response = await invoke<SessionStartResponse>("start_pty_session", {
        cols: terminal.cols,
        rows: terminal.rows,
      });
      sessionIdRef.current = response.sessionId;
      setCurrentOs(response.currentOs);
      await invoke("resize_pty", {
        cols: terminal.cols,
        rows: terminal.rows,
      });
      if (modeRef.current === "translate") {
        writeTerminalAndScroll(terminal, TRANSLATE_PROMPT);
        localPromptVisibleRef.current = true;
      }
    } catch (error) {
      writeTerminalLineAndScroll(
        terminal,
        `Failed to start PTY session: ${String(error)}`,
      );
    }
  };

  const clearTerminal = () => {
    const terminal = terminalRef.current;
    if (!terminal) {
      return;
    }

    terminal.clear();
    terminal.scrollToBottom();

    if (modeRef.current === "translate") {
      translateBufferRef.current = "";
      confirmationBufferRef.current = "";
      awaitingConfirmationRef.current = false;
      localPromptVisibleRef.current = false;
      writeTerminalAndScroll(terminal, TRANSLATE_PROMPT);
      localPromptVisibleRef.current = true;
      return;
    }

    if (sessionIdRef.current !== null) {
      void invoke("write_to_pty", { input: "\n" }).catch(() => undefined);
    }
  };

  const cancelConfirmationFromToolbar = async () => {
    if (!awaitingConfirmationRef.current) {
      return;
    }

    awaitingConfirmationRef.current = false;
    confirmationBufferRef.current = "";
    localPromptVisibleRef.current = false;

    try {
      await invoke<ConfirmationResolutionResponse>("resolve_confirmation", {
        confirmed: false,
      });
    } catch {
      // Ignore toolbar cancellation races during session restarts.
    }
  };

  return (
    <main className="app-shell">
      <section className="terminal-frame">
        <header className="frame-bar">
          <div className="frame-title-block">
            <div className="traffic-lights" aria-hidden="true">
              <span className="dot dot-red" />
              <span className="dot dot-yellow" />
              <span className="dot dot-green" />
            </div>
            <div>
              <div className="frame-title">CLI4ALL</div>
              <div className="frame-subtitle">
                PTY-backed desktop terminal for {currentOs}
              </div>
            </div>
          </div>

          <div className="toolbar">
            <div className="mode-pill">Current Mode: {modeLabel(mode)}</div>
            <button className="toolbar-button" type="button" onClick={startNewSession}>
              New Session
            </button>
            <button className="toolbar-button" type="button" onClick={clearTerminal}>
              Clear Terminal
            </button>
            <button
              className="toolbar-button toolbar-button-primary"
              type="button"
              onClick={toggleMode}
            >
              {mode === "native"
                ? "Switch to Translate Mode"
                : "Switch to Native Mode"}
            </button>
          </div>
        </header>
        <div className="terminal-surface" ref={terminalContainerRef} />
      </section>
    </main>
  );
}

function isPrintableInput(data: string): boolean {
  return !/[\u0000-\u001F\u007F-\u009F]/.test(data);
}

function matchesYes(input: string): boolean {
  return matchesWord(input, ["y", "yes"]);
}

function matchesWord(input: string, allowed: string[]): boolean {
  const trimmed = input.trim().toLowerCase();
  return allowed.includes(trimmed);
}

function modeLabel(mode: TerminalMode): string {
  return mode === "native" ? "Native Mode" : "Translate Mode";
}

function printToolbarModeNotice(terminal: Terminal, mode: TerminalMode) {
  writeTerminalAndScroll(terminal, "---------------- CLI4ALL Mode ----------------\r\n");
  writeTerminalAndScroll(terminal, `Switched to ${modeLabel(mode)}\r\n`);
  writeTerminalAndScroll(terminal, `${MODE_RULE}\r\n`);
}

function writeTerminalAndScroll(terminal: Terminal, data: string) {
  terminal.write(data, () => {
    terminal.scrollToBottom();
  });
}

function writeTerminalLineAndScroll(terminal: Terminal, line: string) {
  writeTerminalAndScroll(terminal, `${line}\r\n`);
}

function appendBufferedPtyOutput(current: string, next: string): string {
  const combined = current + next;
  if (combined.length <= BACKGROUND_PTY_BUFFER_LIMIT) {
    return combined;
  }

  return combined.slice(combined.length - BACKGROUND_PTY_BUFFER_LIMIT);
}

function ensureTrailingNewline(stream: string): string {
  return stream.endsWith("\n") ? stream : `${stream}\n`;
}
