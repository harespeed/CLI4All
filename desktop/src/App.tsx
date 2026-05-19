import { invoke } from "@tauri-apps/api/core";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { useEffect, useRef } from "react";

type TerminalAction = "execute" | "confirm" | "block" | "unknown_no_execute";

type TerminalContext = {
  prompt: string;
  currentOs: string;
};

type TerminalResponse = {
  original_command: string;
  detected_source: string;
  current_os: string;
  matched_intent: string | null;
  translated_command: string | null;
  risk_level: string;
  stdout: string;
  stderr: string;
  exit_status: number | null;
  action: TerminalAction;
  risk_reason: string | null;
  message: string | null;
  confirmation_prompt: string | null;
};

const SECTION_RULE = "-----------------------------------------------------";
const OUTPUT_RULE = "------------------------------------------------";
const RESULT_RULE = "--------------------------------------------------";

export default function App() {
  const terminalContainerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!terminalContainerRef.current) {
      return;
    }

    const terminal = new Terminal({
      cursorBlink: true,
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
    terminal.open(terminalContainerRef.current);
    fitAddon.fit();

    let currentInput = "";
    let currentPrompt = "cli4all-terminal> ";
    let busy = false;

    const writePrompt = () => {
      terminal.write(currentPrompt);
    };

    const printLines = (lines: string[]) => {
      lines.forEach((line) => terminal.writeln(line));
    };

    const printTranslation = (response: TerminalResponse) => {
      printLines([
        "---------------- CLI4ALL Translation ----------------",
        `Original command:   ${response.original_command}`,
        `Detected source:    ${response.detected_source}`,
        `Current OS:         ${response.current_os}`,
        `Matched intent:     ${response.matched_intent ?? "unknown"}`,
        `Translated command: ${response.translated_command ?? "unavailable"}`,
        `Risk level:         ${response.risk_level}`,
        SECTION_RULE,
      ]);
    };

    const printOutput = (response: TerminalResponse) => {
      if (!response.stdout && !response.stderr) {
        return;
      }

      printLines(["---------------- Command Output ----------------"]);
      if (response.stdout) {
        terminal.writeln("[stdout]");
        writeStream(terminal, response.stdout);
      }
      if (response.stderr) {
        terminal.writeln("[stderr]");
        writeStream(terminal, response.stderr);
      }
      terminal.writeln(OUTPUT_RULE);
    };

    const printResult = (response: TerminalResponse) => {
      if (response.exit_status === null) {
        return;
      }

      printLines([
        "---------------- Execution Result ----------------",
        `Exit status: ${response.exit_status}`,
        RESULT_RULE,
      ]);
    };

    const printNotice = (title: string, lines: string[]) => {
      printLines([
        `---------------- ${title} ----------------`,
        ...lines,
        SECTION_RULE,
      ]);
    };

    const renderResponse = (response: TerminalResponse) => {
      printTranslation(response);

      switch (response.action) {
        case "execute":
          printOutput(response);
          printResult(response);
          break;
        case "confirm":
          printNotice("CLI4ALL Notice", [
            response.message ?? "Confirmation required before execution.",
            response.confirmation_prompt ?? "Execute this command? [y/N]",
          ]);
          break;
        case "block":
          printNotice("CLI4ALL Safety", [
            response.message ?? "Destructive command blocked by CLI4ALL.",
            response.risk_reason ? `Reason: ${response.risk_reason}` : "Reason: blocked by safety policy.",
          ]);
          break;
        case "unknown_no_execute":
          printNotice("CLI4ALL Notice", [
            response.message ??
              "Unknown command mapping. CLI4ALL will not execute this command automatically in safe mode.",
          ]);
          break;
      }
    };

    const runCommand = async (command: string) => {
      busy = true;
      try {
        const response = await invoke<TerminalResponse>("process_command", {
          input: command,
        });
        renderResponse(response);
      } catch (error) {
        printNotice("CLI4ALL Notice", [
          `Backend error: ${String(error)}`,
        ]);
      } finally {
        busy = false;
        writePrompt();
      }
    };

    const disposable = terminal.onData((data) => {
      if (busy) {
        return;
      }

      switch (data) {
        case "\r": {
          const command = currentInput;
          currentInput = "";
          terminal.write("\r\n");
          if (command.trim().length === 0) {
            writePrompt();
            return;
          }
          void runCommand(command);
          return;
        }
        case "\u007F": {
          if (currentInput.length > 0) {
            currentInput = currentInput.slice(0, -1);
            terminal.write("\b \b");
          }
          return;
        }
        case "\u0003": {
          currentInput = "";
          terminal.write("^C\r\n");
          writePrompt();
          return;
        }
        default: {
          if (isPrintableInput(data)) {
            currentInput += data;
            terminal.write(data);
          }
        }
      }
    });

    const handleResize = () => {
      fitAddon.fit();
    };

    window.addEventListener("resize", handleResize);

    invoke<TerminalContext>("get_terminal_context")
      .then((context) => {
        currentPrompt = context.prompt;
        terminal.writeln(`CLI4ALL desktop terminal (${context.currentOs})`);
        writePrompt();
      })
      .catch((error) => {
        terminal.writeln(`Failed to load terminal context: ${String(error)}`);
        writePrompt();
      });

    return () => {
      disposable.dispose();
      window.removeEventListener("resize", handleResize);
      terminal.dispose();
    };
  }, []);

  return (
    <main className="app-shell">
      <section className="terminal-frame">
        <header className="frame-bar">
          <div className="traffic-lights" aria-hidden="true">
            <span className="dot dot-red" />
            <span className="dot dot-yellow" />
            <span className="dot dot-green" />
          </div>
          <div className="frame-title">CLI4ALL Desktop Terminal</div>
        </header>
        <div className="terminal-surface" ref={terminalContainerRef} />
      </section>
    </main>
  );
}

function isPrintableInput(data: string) {
  return data >= " " && data !== "\u007f";
}

function writeStream(terminal: Terminal, value: string) {
  const normalized = value.replace(/\r?\n/g, "\r\n");
  terminal.write(normalized);
  if (!normalized.endsWith("\r\n")) {
    terminal.write("\r\n");
  }
}
