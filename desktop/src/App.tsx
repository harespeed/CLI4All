import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import type { CSSProperties, ChangeEvent } from "react";
import { useEffect, useRef, useState } from "react";

type TerminalMode = "native" | "translate";
type DetailMode = "clean" | "verbose";
type BackgroundMode = "color" | "image";
type TerminalAction = "execute" | "confirm" | "block" | "unknown_no_execute";
type ConfirmationResolutionAction = "execute" | "cancelled";

type SessionStartResponse = {
  sessionId: number;
  currentOs: string;
  currentDir: string;
  homeDir: string | null;
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
  streamCommandId: number | null;
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
  streamCommandId: number | null;
};

type TranslateOutputEvent = {
  commandId: number;
  stream: "stdout" | "stderr";
  text: string;
};

type TranslateExitEvent = {
  commandId: number;
  exitStatus: number | null;
  interrupted: boolean;
};

type InterruptTranslateCommandResponse = {
  commandId: number | null;
  interrupted: boolean;
};

type CatalogSuggestion = {
  commandTemplate: string;
  intentId: string;
  description: string;
  sourceShell: string;
  targetShell: string;
  risk: "low" | "medium" | "high" | "destructive";
  previewTranslation: string | null;
  score: number;
};

type PtyOutputEvent = {
  sessionId: number;
  data: string;
};

type PtyExitEvent = {
  sessionId: number;
};

type PendingConfirmationDetails = Pick<
  SubmitTerminalLineResponse,
  | "originalCommand"
  | "detectedSource"
  | "currentOs"
  | "translatedCommand"
  | "riskLevel"
  | "matchedIntent"
>;

type AppearanceSettings = {
  backgroundMode: BackgroundMode;
  backgroundColor: string;
  backgroundImage: string | null;
  backgroundOverlayOpacity: number;
  fontFamily: string;
  fontSize: number;
  fontWeight: number;
  italic: boolean;
  promptColor: string;
  modeTagColor: string;
  translationHintColor: string;
  successColor: string;
  warningColor: string;
  errorColor: string;
  stdoutColor: string;
  stderrColor: string;
  noticeColor: string;
  infoColor: string;
  suggestionColor: string;
  terminalForeground: string;
  terminalBackground: string;
  cursorColor: string;
  selectionColor: string;
};

type StyledSegment = {
  text: string;
  color?: string;
  bold?: boolean;
  italic?: boolean;
  dim?: boolean;
};

type TerminalThemeLike = {
  background: string;
  foreground: string;
  cursor: string;
  selectionBackground: string;
  black: string;
  blue: string;
  brightBlack: string;
  brightBlue: string;
  brightCyan: string;
  brightGreen: string;
  brightMagenta: string;
  brightRed: string;
  brightWhite: string;
  brightYellow: string;
  cyan: string;
  green: string;
  magenta: string;
  red: string;
  white: string;
  yellow: string;
};

const MODE_RULE = "------------------------------------------------";
const SECTION_RULE = "-----------------------------------------------------";
const SCROLLBACK_LINES = 20000;
const BOTTOM_SCROLL_THRESHOLD = 2;
const BACKGROUND_PTY_BUFFER_LIMIT = 200_000;
const BUILTIN_SOURCE = "CLI4ALL Built-in";
const APPEARANCE_STORAGE_KEY = "cli4all.desktop.appearance";
const TRANSLATE_HISTORY_STORAGE_KEY = "cli4all.translate.history.v1";
const TRANSLATE_HISTORY_LIMIT = 500;
const CATALOG_SUGGESTION_LIMIT = 5;
const CATALOG_SUGGESTION_DEBOUNCE_MS = 90;
const FONT_FAMILIES = [
  "Menlo",
  "Monaco",
  "SF Mono",
  "Consolas",
  "Fira Code",
  "JetBrains Mono",
  "monospace",
] as const;
const FONT_WEIGHTS = [400, 500, 600, 700] as const;

const DEFAULT_APPEARANCE_SETTINGS: AppearanceSettings = {
  backgroundMode: "color",
  backgroundColor: "#07111c",
  backgroundImage: null,
  backgroundOverlayOpacity: 0.54,
  fontFamily: "Menlo",
  fontSize: 14,
  fontWeight: 500,
  italic: false,
  promptColor: "#ffe27a",
  modeTagColor: "#7ae7d0",
  translationHintColor: "#9dd1ff",
  successColor: "#c0e77d",
  warningColor: "#ffd479",
  errorColor: "#ff8f7a",
  stdoutColor: "#dce7f3",
  stderrColor: "#ffb7ab",
  noticeColor: "#98c8ff",
  infoColor: "#8fb7d6",
  suggestionColor: "#6a7d8f",
  terminalForeground: "#dce7f3",
  terminalBackground: "#07111c",
  cursorColor: "#f8d66d",
  selectionColor: "#5d8fc0",
};

export default function App() {
  const terminalContainerRef = useRef<HTMLDivElement | null>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const sessionIdRef = useRef<number | null>(null);
  const modeRef = useRef<TerminalMode>("native");
  const detailModeRef = useRef<DetailMode>("clean");
  const appearanceRef = useRef<AppearanceSettings>(DEFAULT_APPEARANCE_SETTINGS);
  const translateBufferRef = useRef("");
  const translateGhostRef = useRef("");
  const confirmationBufferRef = useRef("");
  const localPromptVisibleRef = useRef(false);
  const awaitingConfirmationRef = useRef(false);
  const translateCommandRunningRef = useRef(false);
  const activeTranslateCommandIdRef = useRef<number | null>(null);
  const hiddenPtyOutputRef = useRef("");
  const destroyedRef = useRef(false);
  const translateCwdRef = useRef("");
  const translateHomeDirRef = useRef<string | null>(null);
  const pendingConfirmationDetailsRef =
    useRef<PendingConfirmationDetails | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const translateHistoryRef = useRef<string[]>([]);
  const translateHistoryIndexRef = useRef<number | null>(null);
  const translateHistoryDraftRef = useRef("");
  const catalogSuggestionsRef = useRef<CatalogSuggestion[]>([]);
  const catalogSuggestionIndexRef = useRef(0);
  const catalogSearchTimerRef = useRef<number | null>(null);
  const catalogSearchSequenceRef = useRef(0);
  const catalogSuggestionsDismissedRef = useRef(false);

  const [mode, setMode] = useState<TerminalMode>("native");
  const [detailMode, setDetailMode] = useState<DetailMode>("clean");
  const [currentOs, setCurrentOs] = useState("Starting...");
  const [appearance, setAppearance] = useState<AppearanceSettings>(
    loadAppearanceSettings,
  );
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [translateHistory, setTranslateHistory] = useState<string[]>(
    loadTranslateHistory,
  );
  const [catalogSuggestions, setCatalogSuggestions] = useState<CatalogSuggestion[]>(
    [],
  );
  const [catalogSuggestionIndex, setCatalogSuggestionIndex] = useState(0);

  useEffect(() => {
    modeRef.current = mode;
  }, [mode]);

  useEffect(() => {
    detailModeRef.current = detailMode;
  }, [detailMode]);

  useEffect(() => {
    appearanceRef.current = appearance;
  }, [appearance]);

  useEffect(() => {
    translateHistoryRef.current = translateHistory;
  }, [translateHistory]);

  useEffect(() => {
    catalogSuggestionsRef.current = catalogSuggestions;
  }, [catalogSuggestions]);

  useEffect(() => {
    catalogSuggestionIndexRef.current = catalogSuggestionIndex;
  }, [catalogSuggestionIndex]);

  const resetCatalogSuggestionState = () => {
    if (catalogSearchTimerRef.current !== null) {
      window.clearTimeout(catalogSearchTimerRef.current);
      catalogSearchTimerRef.current = null;
    }
    catalogSearchSequenceRef.current += 1;
    catalogSuggestionsDismissedRef.current = false;
    catalogSuggestionIndexRef.current = 0;
    setCatalogSuggestionIndex(0);
    setCatalogSuggestions([]);
  };

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    window.localStorage.setItem(
      APPEARANCE_STORAGE_KEY,
      JSON.stringify(appearance),
    );
  }, [appearance]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    window.localStorage.setItem(
      TRANSLATE_HISTORY_STORAGE_KEY,
      JSON.stringify(translateHistory),
    );
  }, [translateHistory]);

  useEffect(() => {
    const terminal = terminalRef.current;
    if (!terminal) {
      return;
    }

    terminal.options = {
      ...terminal.options,
      fontFamily: appearance.fontFamily,
      fontSize: appearance.fontSize,
      fontWeight: appearance.fontWeight,
      theme: buildXtermTheme(appearance),
    };

    const fitAddon = fitAddonRef.current;
    if (fitAddon) {
      fitAddon.fit();
      if (sessionIdRef.current !== null) {
        void invoke("resize_pty", {
          cols: terminal.cols,
          rows: terminal.rows,
        }).catch(() => undefined);
      }
    }
  }, [appearance]);

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
      fontFamily: appearanceRef.current.fontFamily,
      fontSize: appearanceRef.current.fontSize,
      fontWeight: appearanceRef.current.fontWeight,
      theme: buildXtermTheme(appearanceRef.current),
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

    const writeStyled = (
      segments: StyledSegment[],
      scrollBehavior: "always" | "if-bottom" | "never" = "always",
    ) => {
      writeTerminal(styleSegments(segments), scrollBehavior);
    };

    const writeStyledLine = (
      segments: StyledSegment[],
      scrollBehavior: "always" | "if-bottom" | "never" = "always",
    ) => {
      writeTerminal(`${styleSegments(segments)}\r\n`, scrollBehavior);
    };

    const writeColoredBlock = (
      text: string,
      color: string,
      scrollBehavior: "always" | "if-bottom" | "never" = "always",
    ) => {
      const normalized = ensureTrailingNewline(text).replace(/\n/g, "\r\n");
      writeTerminal(
        `${styleSegments([{ text: normalized, color }])}`,
        scrollBehavior,
      );
    };

    const writeStream = (data: string) => {
      writeTerminal(data, "if-bottom");
    };

    const printVerboseSectionHeader = (title: string, color: string) => {
      writeStyledLine(
        [
          {
            text: `---------------- ${title} ----------------`,
            color,
            bold: true,
          },
        ],
        "always",
      );
    };

    const printVerboseSectionFooter = (rule: string, color: string) => {
      writeStyledLine([{ text: rule, color }], "always");
    };

    const printVerboseNotice = (title: string, lines: string[]) => {
      const settings = appearanceRef.current;
      printVerboseSectionHeader(title, settings.infoColor);
      lines.forEach((line) => {
        writeStyledLine([{ text: line, color: settings.noticeColor }], "always");
      });
      printVerboseSectionFooter(SECTION_RULE, settings.infoColor);
    };

    const printCompactNotice = (message: string, color: string) => {
      writeStyledLine([{ text: message, color, bold: true }], "always");
    };

    const printNotice = (title: string, lines: string[], compactMessage: string) => {
      const settings = appearanceRef.current;
      if (detailModeRef.current === "verbose") {
        printVerboseNotice(title, lines);
      } else {
        const color =
          title === "CLI4ALL Safety"
            ? settings.errorColor
            : settings.noticeColor;
        printCompactNotice(compactMessage, color);
      }
    };

    const printTranslationVerbose = (response: SubmitTerminalLineResponse) => {
      const settings = appearanceRef.current;
      printVerboseSectionHeader("CLI4ALL Translation", settings.translationHintColor);
      printTranslationDetailLine("Original command", response.originalCommand);
      printTranslationDetailLine("Detected source", response.detectedSource);
      printTranslationDetailLine("Current OS", response.currentOs);
      printTranslationDetailLine(
        "Matched intent",
        response.matchedIntent ?? "unknown",
      );
      printTranslationDetailLine(
        "Translated command",
        response.translatedCommand ?? "unavailable",
      );
      printTranslationDetailLine("Risk level", response.riskLevel);
      printVerboseSectionFooter(SECTION_RULE, settings.translationHintColor);
    };

    const printTranslationDetailLine = (label: string, value: string) => {
      const settings = appearanceRef.current;
      writeStyledLine(
        [
          { text: `${label}: `, color: settings.infoColor, bold: true },
          { text: value, color: settings.translationHintColor },
        ],
        "always",
      );
    };

    const printExecutionVerbose = (
      stdout: string,
      stderr: string,
      exitStatus: number | null,
    ) => {
      const settings = appearanceRef.current;

      if (stdout.length > 0 || stderr.length > 0) {
        printVerboseSectionHeader("Command Output", settings.infoColor);

        if (stdout.length > 0) {
          writeStyledLine(
            [{ text: "[stdout]", color: settings.stdoutColor, bold: true }],
            "always",
          );
          writeColoredBlock(stdout, settings.stdoutColor, "always");
        }

        if (stderr.length > 0) {
          writeStyledLine(
            [{ text: "[stderr]", color: settings.stderrColor, bold: true }],
            "always",
          );
          writeColoredBlock(stderr, settings.stderrColor, "always");
        }

        printVerboseSectionFooter("------------------------------------------------", settings.infoColor);
      }

      printVerboseSectionHeader("Execution Result", settings.infoColor);
      writeStyledLine(
        [
          {
            text: `Exit status: ${exitStatus ?? "unavailable"}`,
            color: exitStatus === 0 ? settings.successColor : settings.errorColor,
            bold: true,
          },
        ],
        "always",
      );
      printVerboseSectionFooter("--------------------------------------------------", settings.infoColor);
    };

    const printTranslationHint = (
      source: string,
      originalCommand: string,
      targetOs: string,
      translatedCommand: string,
    ) => {
      const settings = appearanceRef.current;
      writeStyledLine(
        [
          { text: "✓ ", color: settings.successColor, bold: true },
          {
            text: `${source}: ${originalCommand} → ${targetOs}: ${translatedCommand}`,
            color: settings.translationHintColor,
            bold: true,
          },
        ],
        "always",
      );
    };

    const printExecutionClean = (
      stdout: string,
      stderr: string,
      exitStatus: number | null,
    ) => {
      const settings = appearanceRef.current;

      if (stdout.length > 0) {
        writeColoredBlock(stdout, settings.stdoutColor, "always");
      }

      if (stderr.length > 0) {
        writeStyledLine(
          [{ text: "[stderr]", color: settings.stderrColor, bold: true }],
          "always",
        );
        writeColoredBlock(stderr, settings.stderrColor, "always");
      }

      if (exitStatus !== null && exitStatus !== 0) {
        writeStyledLine(
          [
            {
              text: `✗ Exit status: ${exitStatus}`,
              color: settings.errorColor,
              bold: true,
            },
          ],
          "always",
        );
      }
    };

    const resetLocalState = () => {
      translateBufferRef.current = "";
      translateGhostRef.current = "";
      confirmationBufferRef.current = "";
      localPromptVisibleRef.current = false;
      awaitingConfirmationRef.current = false;
      translateCommandRunningRef.current = false;
      activeTranslateCommandIdRef.current = null;
      pendingConfirmationDetailsRef.current = null;
      translateHistoryIndexRef.current = null;
      translateHistoryDraftRef.current = "";
      catalogSuggestionsDismissedRef.current = false;
      clearCatalogSuggestions();
    };

    const renderTranslatePromptLine = (prependNewline: boolean) => {
      if (modeRef.current !== "translate") {
        return;
      }
      if (awaitingConfirmationRef.current) {
        return;
      }

      if (prependNewline) {
        writeTerminal("\r\n", "always");
      }

      writeTerminal(
        `\r\x1b[2K${styleSegments([
          ...buildTranslatePromptSegments(
            translateCwdRef.current,
            translateHomeDirRef.current,
            appearanceRef.current,
          ),
          {
            text: translateBufferRef.current,
            color: appearanceRef.current.stdoutColor,
          },
          ...(translateGhostRef.current
            ? [
                {
                  text: translateGhostRef.current,
                  color: appearanceRef.current.suggestionColor,
                  dim: true,
                } satisfies StyledSegment,
              ]
            : []),
        ])}`,
        "always",
      );
      localPromptVisibleRef.current = true;
    };

    const showTranslatePrompt = (prependNewline: boolean) => {
      if (modeRef.current !== "translate") {
        return;
      }
      if (awaitingConfirmationRef.current || localPromptVisibleRef.current) {
        return;
      }

      renderTranslatePromptLine(prependNewline);
    };

    const syncTranslateGhost = () => {
      translateGhostRef.current = findGhostSuggestion(
        translateBufferRef.current,
        translateHistoryRef.current,
        translateHistoryIndexRef.current,
      );
    };

    const redrawTranslatePromptLine = () => {
      syncTranslateGhost();
      if (translateGhostRef.current.length > 0) {
        clearCatalogSuggestions();
      }
      if (localPromptVisibleRef.current) {
        renderTranslatePromptLine(false);
      } else {
        showTranslatePrompt(false);
      }
      scheduleCatalogSuggestionSearch();
    };

    const saveTranslateHistoryEntry = (input: string) => {
      setTranslateHistory((current) => pushTranslateHistoryEntry(current, input));
    };

    const clearCatalogSuggestions = () => {
      if (catalogSearchTimerRef.current !== null) {
        window.clearTimeout(catalogSearchTimerRef.current);
        catalogSearchTimerRef.current = null;
      }
      catalogSearchSequenceRef.current += 1;
      catalogSuggestionIndexRef.current = 0;
      setCatalogSuggestionIndex(0);
      setCatalogSuggestions([]);
    };

    const scheduleCatalogSuggestionSearch = () => {
      if (
        modeRef.current !== "translate" ||
        awaitingConfirmationRef.current ||
        translateCommandRunningRef.current ||
        translateHistoryIndexRef.current !== null
      ) {
        clearCatalogSuggestions();
        return;
      }

      const query = translateBufferRef.current.trim();
      if (
        query.length === 0 ||
        translateGhostRef.current.length > 0 ||
        catalogSuggestionsDismissedRef.current
      ) {
        clearCatalogSuggestions();
        return;
      }

      if (catalogSearchTimerRef.current !== null) {
        window.clearTimeout(catalogSearchTimerRef.current);
      }

      const sequence = catalogSearchSequenceRef.current + 1;
      catalogSearchSequenceRef.current = sequence;
      catalogSearchTimerRef.current = window.setTimeout(() => {
        catalogSearchTimerRef.current = null;
        void invoke<CatalogSuggestion[]>("search_catalog_suggestions", {
          query,
          limit: CATALOG_SUGGESTION_LIMIT,
        })
          .then((results) => {
            if (sequence !== catalogSearchSequenceRef.current) {
              return;
            }

            setCatalogSuggestions(results);
            const nextIndex = results.length > 0 ? 0 : 0;
            catalogSuggestionIndexRef.current = nextIndex;
            setCatalogSuggestionIndex(nextIndex);
          })
          .catch(() => {
            if (sequence !== catalogSearchSequenceRef.current) {
              return;
            }
            setCatalogSuggestions([]);
            catalogSuggestionIndexRef.current = 0;
            setCatalogSuggestionIndex(0);
          });
      }, CATALOG_SUGGESTION_DEBOUNCE_MS);
    };

    const navigateTranslateHistory = (direction: "up" | "down") => {
      const history = translateHistoryRef.current;
      if (history.length === 0) {
        return;
      }

      if (translateHistoryIndexRef.current === null) {
        if (direction === "down") {
          return;
        }
        translateHistoryDraftRef.current = translateBufferRef.current;
        translateHistoryIndexRef.current = history.length - 1;
      } else if (direction === "up") {
        translateHistoryIndexRef.current = Math.max(
          0,
          translateHistoryIndexRef.current - 1,
        );
      } else if (translateHistoryIndexRef.current >= history.length - 1) {
        translateHistoryIndexRef.current = null;
        translateBufferRef.current = translateHistoryDraftRef.current;
        translateHistoryDraftRef.current = "";
        redrawTranslatePromptLine();
        return;
      } else {
        translateHistoryIndexRef.current += 1;
      }

      if (translateHistoryIndexRef.current !== null) {
        translateBufferRef.current = history[translateHistoryIndexRef.current] ?? "";
      }
      catalogSuggestionsDismissedRef.current = false;
      clearCatalogSuggestions();
      redrawTranslatePromptLine();
    };

    const acceptTranslateGhostSuggestion = () => {
      if (translateGhostRef.current.length === 0) {
        return false;
      }

      translateBufferRef.current += translateGhostRef.current;
      translateGhostRef.current = "";
      translateHistoryIndexRef.current = null;
      catalogSuggestionsDismissedRef.current = false;
      clearCatalogSuggestions();
      redrawTranslatePromptLine();
      return true;
    };

    const moveCatalogSuggestionSelection = (direction: 1 | -1) => {
      const suggestions = catalogSuggestionsRef.current;
      if (suggestions.length === 0) {
        return;
      }

      const nextIndex =
        (catalogSuggestionIndexRef.current + direction + suggestions.length) %
        suggestions.length;
      catalogSuggestionIndexRef.current = nextIndex;
      setCatalogSuggestionIndex(nextIndex);
    };

    const acceptSelectedCatalogSuggestion = () => {
      const suggestions = catalogSuggestionsRef.current;
      if (suggestions.length === 0) {
        return false;
      }

      const selected =
        suggestions[catalogSuggestionIndexRef.current] ?? suggestions[0];
      if (!selected) {
        return false;
      }

      translateBufferRef.current = selected.commandTemplate;
      translateGhostRef.current = "";
      translateHistoryIndexRef.current = null;
      translateHistoryDraftRef.current = "";
      catalogSuggestionsDismissedRef.current = false;
      clearCatalogSuggestions();
      redrawTranslatePromptLine();
      return true;
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

    const renderCleanTranslateExecution = (
      response: Pick<
        SubmitTerminalLineResponse,
        | "originalCommand"
        | "detectedSource"
        | "currentOs"
        | "translatedCommand"
        | "stdout"
        | "stderr"
        | "exitStatus"
        | "matchedIntent"
      >,
    ) => {
      if (shouldPrintCompactTranslationHint(response)) {
        printTranslationHint(
          response.detectedSource,
          response.originalCommand,
          response.currentOs,
          response.translatedCommand ?? "unavailable",
        );
      }

      printExecutionClean(response.stdout, response.stderr, response.exitStatus);
      showTranslatePrompt(false);
    };

    const writeTranslateStreamChunk = (
      stream: "stdout" | "stderr",
      text: string,
    ) => {
      const normalized = text.replace(/\r?\n/g, "\r\n");
      writeTerminal(
        styleSegments([
          {
            text: normalized,
            color:
              stream === "stderr"
                ? appearanceRef.current.stderrColor
                : appearanceRef.current.stdoutColor,
          },
        ]),
        "if-bottom",
      );
    };

    const handleTranslateCommandExit = (event: TranslateExitEvent) => {
      if (event.commandId !== activeTranslateCommandIdRef.current) {
        return;
      }

      translateCommandRunningRef.current = false;
      activeTranslateCommandIdRef.current = null;
      clearCatalogSuggestions();

      if (event.interrupted) {
        writeStyledLine(
          [{ text: "Command interrupted.", color: appearanceRef.current.noticeColor }],
          "always",
        );
        showTranslatePrompt(false);
        return;
      }

      if (detailModeRef.current === "verbose") {
        printVerboseSectionFooter(
          "------------------------------------------------",
          appearanceRef.current.infoColor,
        );
        printExecutionVerbose("", "", event.exitStatus);
        showTranslatePrompt(false);
        return;
      }

      if (event.exitStatus !== null && event.exitStatus !== 0) {
        writeStyledLine(
          [
            {
              text: `✗ Exit status: ${event.exitStatus}`,
              color: appearanceRef.current.errorColor,
              bold: true,
            },
          ],
          "always",
        );
      }

      showTranslatePrompt(false);
    };

    const handleSubmitResponse = (response: SubmitTerminalLineResponse) => {
      translateCwdRef.current = response.currentDir;
      clearCatalogSuggestions();

      if (response.clearDisplay) {
        terminal.clear();
        terminal.scrollToBottom();
      }

      if (detailModeRef.current === "verbose") {
        printTranslationVerbose(response);
      }

      switch (response.action) {
        case "execute":
          if (response.streamCommandId !== null) {
            translateCommandRunningRef.current = true;
            activeTranslateCommandIdRef.current = response.streamCommandId;
            if (detailModeRef.current === "verbose") {
              printVerboseSectionHeader(
                "Command Output",
                appearanceRef.current.infoColor,
              );
            } else if (shouldPrintCompactTranslationHint(response)) {
              printTranslationHint(
                response.detectedSource,
                response.originalCommand,
                response.currentOs,
                response.translatedCommand ?? "unavailable",
              );
            }
          } else if (detailModeRef.current === "verbose") {
            printExecutionVerbose(
              response.stdout,
              response.stderr,
              response.exitStatus,
            );
            showTranslatePrompt(false);
          } else {
            renderCleanTranslateExecution(response);
          }
          break;
        case "confirm":
          pendingConfirmationDetailsRef.current = {
            originalCommand: response.originalCommand,
            detectedSource: response.detectedSource,
            currentOs: response.currentOs,
            translatedCommand: response.translatedCommand,
            riskLevel: response.riskLevel,
            matchedIntent: response.matchedIntent,
          };
          awaitingConfirmationRef.current = true;
          confirmationBufferRef.current = "";
          writeStyledLine(
            [
              {
                text:
                  detailModeRef.current === "verbose"
                    ? response.confirmationPrompt ??
                      buildCompactConfirmationPrompt(response.riskLevel)
                    : buildCompactConfirmationPrompt(response.riskLevel),
                color: appearanceRef.current.warningColor,
                bold: true,
              },
            ],
            "always",
          );
          break;
        case "block":
          printNotice(
            "CLI4ALL Safety",
            [
              response.message ?? "Destructive command blocked by CLI4ALL.",
              response.riskReason
                ? `Reason: ${response.riskReason}`
                : "Reason: blocked by safety policy.",
            ],
            "✗ Blocked destructive command.",
          );
          showTranslatePrompt(false);
          break;
        case "unknown_no_execute":
          printNotice(
            "CLI4ALL Notice",
            [
              response.message ??
                "Unknown command mapping. CLI4ALL will not execute this command automatically in safe mode.",
            ],
            "? Unknown command mapping. Not executed.",
          );
          showTranslatePrompt(false);
          break;
      }
    };

    const submitTranslateLine = async () => {
      const input = translateBufferRef.current;
      translateBufferRef.current = "";
      translateGhostRef.current = "";
      translateHistoryIndexRef.current = null;
      translateHistoryDraftRef.current = "";
      localPromptVisibleRef.current = false;
      clearCatalogSuggestions();
      writeTerminal("\r\n", "always");

      if (input.trim().length === 0) {
        showTranslatePrompt(false);
        return;
      }

      saveTranslateHistoryEntry(input);

      try {
        const response = await invoke<SubmitTerminalLineResponse>(
          "submit_terminal_line",
          {
            input,
          },
        );
        handleSubmitResponse(response);
      } catch (error) {
        printNotice(
          "CLI4ALL Notice",
          [`Backend error: ${String(error)}`],
          `? Backend error: ${String(error)}`,
        );
        showTranslatePrompt(false);
      }
    };

    const resolveConfirmation = async () => {
      const approved = matchesYes(confirmationBufferRef.current);
      const pendingDetails = pendingConfirmationDetailsRef.current;

      confirmationBufferRef.current = "";
      awaitingConfirmationRef.current = false;
      localPromptVisibleRef.current = false;
      pendingConfirmationDetailsRef.current = null;
      clearCatalogSuggestions();
      writeTerminal("\r\n", "always");

      try {
        const response = await invoke<ConfirmationResolutionResponse>(
          "resolve_confirmation",
          {
            confirmed: approved,
          },
        );

        translateCwdRef.current = response.currentDir;

        if (response.action === "cancelled") {
          if (detailModeRef.current === "verbose") {
            printVerboseNotice("CLI4ALL Notice", [response.message]);
          } else {
            writeStyledLine(
              [{ text: response.message, color: appearanceRef.current.noticeColor }],
              "always",
            );
          }
          showTranslatePrompt(false);
          return;
        }

        if (response.clearDisplay) {
          terminal.clear();
          terminal.scrollToBottom();
        }

        if (detailModeRef.current === "verbose") {
          printExecutionVerbose(
            response.stdout,
            response.stderr,
            response.exitStatus,
          );
          showTranslatePrompt(false);
          return;
        }

        if (
          pendingDetails &&
          shouldPrintCompactTranslationHint(pendingDetails)
        ) {
          printTranslationHint(
            pendingDetails.detectedSource,
            pendingDetails.originalCommand,
            pendingDetails.currentOs,
            pendingDetails.translatedCommand ?? "unavailable",
          );
        }

        if (response.streamCommandId !== null) {
          translateCommandRunningRef.current = true;
          activeTranslateCommandIdRef.current = response.streamCommandId;
          return;
        }

        printExecutionClean(response.stdout, response.stderr, response.exitStatus);
        showTranslatePrompt(false);
      } catch (error) {
        printNotice(
          "CLI4ALL Notice",
          [`Backend error: ${String(error)}`],
          `? Backend error: ${String(error)}`,
        );
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
        translateCwdRef.current = response.currentDir;
        translateHomeDirRef.current = response.homeDir;
        await syncPtySize();

        if (modeRef.current === "translate") {
          showTranslatePrompt(false);
        }
      } catch (error) {
        printNotice(
          "CLI4ALL Notice",
          [`Failed to start PTY session: ${String(error)}`],
          `? Failed to start PTY session: ${String(error)}`,
        );
      }
    };

    const handleNativeInput = (data: string) => {
      terminal.scrollToBottom();
      void invoke("write_to_pty", { input: data }).catch((error) => {
        printNotice(
          "CLI4ALL Notice",
          [`Backend error: ${String(error)}`],
          `? Backend error: ${String(error)}`,
        );
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
            writeStyledLine(
              [{ text: "^C", color: appearanceRef.current.warningColor, bold: true }],
              "always",
            );
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

      if (translateCommandRunningRef.current) {
        if (data === "\u0003") {
          writeStyledLine(
            [{ text: "^C", color: appearanceRef.current.warningColor, bold: true }],
            "always",
          );
          void invoke<InterruptTranslateCommandResponse>(
            "interrupt_translate_command",
          ).catch((error) => {
            printNotice(
              "CLI4ALL Notice",
              [`Backend error: ${String(error)}`],
              `? Backend error: ${String(error)}`,
            );
          });
        }
        return;
      }

      switch (data) {
        case "\u001b[A":
          terminal.scrollToBottom();
          navigateTranslateHistory("up");
          return;
        case "\u001b[B":
          terminal.scrollToBottom();
          navigateTranslateHistory("down");
          return;
        case "\u0010":
          moveCatalogSuggestionSelection(-1);
          return;
        case "\u000E":
          moveCatalogSuggestionSelection(1);
          return;
        case "\t":
          terminal.scrollToBottom();
          if (!acceptTranslateGhostSuggestion()) {
            acceptSelectedCatalogSuggestion();
          }
          return;
        case "\u001b[C":
          if (
            acceptTranslateGhostSuggestion() ||
            (catalogSuggestionsRef.current.length > 0 &&
              translateBufferRef.current.length > 0 &&
              (catalogSuggestionsRef.current[catalogSuggestionIndexRef.current]
                ?.commandTemplate
                .toLowerCase()
                .startsWith(translateBufferRef.current.toLowerCase()) ??
                false) &&
              acceptSelectedCatalogSuggestion())
          ) {
            terminal.scrollToBottom();
          }
          return;
        case "\u001b":
          catalogSuggestionsDismissedRef.current = true;
          clearCatalogSuggestions();
          redrawTranslatePromptLine();
          return;
        case "\r":
          void submitTranslateLine();
          return;
        case "\u007F":
          if (translateBufferRef.current.length > 0) {
            terminal.scrollToBottom();
            translateBufferRef.current = translateBufferRef.current.slice(0, -1);
            translateHistoryIndexRef.current = null;
            catalogSuggestionsDismissedRef.current = false;
            redrawTranslatePromptLine();
          } else {
            clearCatalogSuggestions();
          }
          return;
        case "\u0003":
          if (translateBufferRef.current.length > 0 || localPromptVisibleRef.current) {
            translateBufferRef.current = "";
            translateGhostRef.current = "";
            translateHistoryIndexRef.current = null;
            translateHistoryDraftRef.current = "";
            localPromptVisibleRef.current = false;
            clearCatalogSuggestions();
            writeStyledLine(
              [{ text: "^C", color: appearanceRef.current.warningColor, bold: true }],
              "always",
            );
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
          translateHistoryIndexRef.current = null;
          catalogSuggestionsDismissedRef.current = false;
          redrawTranslatePromptLine();
      }
    };

    const cancelPendingConfirmation = async (printCancellation: boolean) => {
      if (!awaitingConfirmationRef.current) {
        return;
      }

      awaitingConfirmationRef.current = false;
      confirmationBufferRef.current = "";
      localPromptVisibleRef.current = false;
      pendingConfirmationDetailsRef.current = null;

      try {
        await invoke<ConfirmationResolutionResponse>("resolve_confirmation", {
          confirmed: false,
        });
      } catch (error) {
        printNotice(
          "CLI4ALL Notice",
          [`Backend error: ${String(error)}`],
          `? Backend error: ${String(error)}`,
        );
        return;
      }

      if (printCancellation) {
        if (detailModeRef.current === "verbose") {
          printVerboseNotice("CLI4ALL Notice", ["Execution cancelled."]);
        } else {
          writeStyledLine(
            [{ text: "Execution cancelled.", color: appearanceRef.current.noticeColor }],
            "always",
          );
        }
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
    let unlistenTranslateOutput: (() => void) | undefined;
    let unlistenTranslateExit: (() => void) | undefined;

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
      printNotice(
        "CLI4ALL Notice",
        ["PTY session ended. Use New Session to start another terminal."],
        "[CLI4ALL] PTY session ended. Use New Session to start another terminal.",
      );
    }).then((unlisten) => {
      unlistenExit = unlisten;
    });

    void listen<TranslateOutputEvent>("translate-output", (event) => {
      if (destroyedRef.current) {
        return;
      }
      if (event.payload.commandId !== activeTranslateCommandIdRef.current) {
        return;
      }
      writeTranslateStreamChunk(event.payload.stream, event.payload.text);
    }).then((unlisten) => {
      unlistenTranslateOutput = unlisten;
    });

    void listen<TranslateExitEvent>("translate-exit", (event) => {
      if (destroyedRef.current) {
        return;
      }
      handleTranslateCommandExit(event.payload);
    }).then((unlisten) => {
      unlistenTranslateExit = unlisten;
    });

    void startSession();

    return () => {
      destroyedRef.current = true;
      if (catalogSearchTimerRef.current !== null) {
        window.clearTimeout(catalogSearchTimerRef.current);
        catalogSearchTimerRef.current = null;
      }
      dataDisposable.dispose();
      resizeObserver.disconnect();
      window.removeEventListener("resize", handleWindowResize);
      void invoke("stop_pty_session").catch(() => undefined);
      unlistenOutput?.();
      unlistenExit?.();
      unlistenTranslateOutput?.();
      unlistenTranslateExit?.();
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
    translateGhostRef.current = "";
    confirmationBufferRef.current = "";
    localPromptVisibleRef.current = false;
    awaitingConfirmationRef.current = false;
    pendingConfirmationDetailsRef.current = null;
    translateHistoryIndexRef.current = null;
    translateHistoryDraftRef.current = "";
    resetCatalogSuggestionState();

    const nextMode: TerminalMode =
      modeRef.current === "native" ? "translate" : "native";
    modeRef.current = nextMode;
    setMode(nextMode);

    terminal.scrollToBottom();
    writeTerminalAndScroll(terminal, "\r\n");
    printModeSwitchNotice(terminal, nextMode, detailModeRef.current, appearanceRef.current);

    if (nextMode === "translate") {
      terminal.focus();
      writeTerminalAndScroll(
        terminal,
        styleSegments(
          buildTranslatePromptSegments(
            translateCwdRef.current,
            translateHomeDirRef.current,
            appearanceRef.current,
          ),
        ),
      );
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
    translateGhostRef.current = "";
    confirmationBufferRef.current = "";
    localPromptVisibleRef.current = false;
    awaitingConfirmationRef.current = false;
    hiddenPtyOutputRef.current = "";
    pendingConfirmationDetailsRef.current = null;
    translateHistoryIndexRef.current = null;
    translateHistoryDraftRef.current = "";
    resetCatalogSuggestionState();

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
      translateCwdRef.current = response.currentDir;
      translateHomeDirRef.current = response.homeDir;
      await invoke("resize_pty", {
        cols: terminal.cols,
        rows: terminal.rows,
      });
      if (modeRef.current === "translate") {
        writeTerminalAndScroll(
          terminal,
          styleSegments(
            buildTranslatePromptSegments(
              translateCwdRef.current,
              translateHomeDirRef.current,
              appearanceRef.current,
            ),
          ),
        );
        localPromptVisibleRef.current = true;
      }
    } catch (error) {
      writeTerminalLineAndScroll(
        terminal,
        `Failed to start PTY session: ${String(error)}`,
      );
    }
  };

  const clearTerminal = async () => {
    const terminal = terminalRef.current;
    if (!terminal) {
      return;
    }

    await cancelConfirmationFromToolbar();

    terminal.clear();
    terminal.scrollToBottom();

    if (modeRef.current === "translate") {
      translateBufferRef.current = "";
      translateGhostRef.current = "";
      confirmationBufferRef.current = "";
      awaitingConfirmationRef.current = false;
      localPromptVisibleRef.current = false;
      pendingConfirmationDetailsRef.current = null;
      translateHistoryIndexRef.current = null;
      translateHistoryDraftRef.current = "";
      resetCatalogSuggestionState();
      writeTerminalAndScroll(
        terminal,
        styleSegments(
          buildTranslatePromptSegments(
            translateCwdRef.current,
            translateHomeDirRef.current,
            appearanceRef.current,
          ),
        ),
      );
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
    pendingConfirmationDetailsRef.current = null;
    translateGhostRef.current = "";
    translateHistoryIndexRef.current = null;
    translateHistoryDraftRef.current = "";
    resetCatalogSuggestionState();

    try {
      await invoke<ConfirmationResolutionResponse>("resolve_confirmation", {
        confirmed: false,
      });
    } catch {
      // Ignore toolbar cancellation races during session restarts.
    }
  };

  const toggleDetailMode = () => {
    setDetailMode((current) => (current === "clean" ? "verbose" : "clean"));
  };

  const updateAppearance = <K extends keyof AppearanceSettings>(
    key: K,
    value: AppearanceSettings[K],
  ) => {
    setAppearance((current) => ({
      ...current,
      [key]: value,
    }));
  };

  const handleBackgroundImagePick = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) {
      return;
    }

    const reader = new FileReader();
    reader.onload = () => {
      const result = typeof reader.result === "string" ? reader.result : null;
      if (!result) {
        return;
      }

      setAppearance((current) => ({
        ...current,
        backgroundMode: "image",
        backgroundImage: result,
      }));
    };
    reader.readAsDataURL(file);
    event.target.value = "";
  };

  const resetBackgroundImage = () => {
    setAppearance((current) => ({
      ...current,
      backgroundMode: "color",
      backgroundImage: null,
    }));
  };

  const resetAppearance = () => {
    setAppearance(DEFAULT_APPEARANCE_SETTINGS);
  };

  const terminalSurfaceStyle = buildTerminalSurfaceStyle(appearance);
  const terminalViewportStyle: CSSProperties & Record<string, string> = {
    "--terminal-font-style": appearance.italic ? "italic" : "normal",
  };

  return (
    <main className="app-shell">
      <section className="terminal-frame">
        <header className="frame-bar">
          <div className="frame-title-block">
            <div className="frame-title">CLI4ALL</div>
            <div className="frame-subtitle">
              Cross-platform command translation terminal
            </div>
          </div>

          <div className="toolbar">
            <div
              className="mode-pill"
              style={{ color: appearance.modeTagColor, borderColor: `${appearance.modeTagColor}44` }}
            >
              {modeLabel(mode)}
            </div>
            <div className="mode-pill">Output: {detailModeLabel(detailMode)}</div>
            <div className="toolbar-spacer" />
            <button className="toolbar-button" type="button" onClick={startNewSession}>
              New Session
            </button>
            <button className="toolbar-button" type="button" onClick={clearTerminal}>
              Clear Terminal
            </button>
            <button className="toolbar-button" type="button" onClick={toggleDetailMode}>
              Verbose: {detailMode === "verbose" ? "On" : "Off"}
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
            <button
              className="toolbar-button toolbar-button-settings"
              type="button"
              onClick={() => setIsSettingsOpen(true)}
            >
              Settings
            </button>
          </div>
        </header>
        <div className="terminal-surface" style={terminalSurfaceStyle}>
          <div className="terminal-background-overlay" />
          <div
            className="terminal-viewport"
            ref={terminalContainerRef}
            style={terminalViewportStyle}
          />
          {mode === "translate" && catalogSuggestions.length > 0 ? (
            <div className="catalog-suggestions" aria-live="polite">
              {catalogSuggestions.map((suggestion, index) => (
                <div
                  key={`${suggestion.sourceShell}-${suggestion.commandTemplate}`}
                  className={`catalog-suggestion-item${
                    index === catalogSuggestionIndex
                      ? " catalog-suggestion-item-selected"
                      : ""
                  }`}
                  style={buildCatalogSuggestionItemStyle(
                    appearance,
                    suggestion,
                    index === catalogSuggestionIndex,
                  )}
                >
                  <div className="catalog-suggestion-line">
                    <span className="catalog-suggestion-index">{index + 1}</span>
                    <span className="catalog-suggestion-command">
                      {suggestion.commandTemplate}
                    </span>
                    {suggestion.risk !== "low" ? (
                      <span
                        className="catalog-suggestion-risk"
                        style={{
                          color:
                            suggestion.risk === "high"
                              ? appearance.errorColor
                              : appearance.warningColor,
                        }}
                      >
                        {suggestion.risk}
                      </span>
                    ) : null}
                  </div>
                  <div className="catalog-suggestion-meta">
                    {formatCatalogSuggestionMeta(suggestion)}
                  </div>
                </div>
              ))}
            </div>
          ) : null}
        </div>
      </section>

      {isSettingsOpen ? (
        <div
          className="settings-backdrop"
          role="presentation"
          onClick={() => setIsSettingsOpen(false)}
        >
          <aside
            className="settings-panel"
            role="dialog"
            aria-modal="true"
            aria-label="Appearance settings"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="settings-header">
              <div>
                <div className="settings-title">Appearance Settings</div>
                <div className="settings-subtitle">
                  Customize the CLI4ALL terminal look without changing terminal behavior.
                </div>
              </div>
              <button
                className="toolbar-button"
                type="button"
                onClick={() => setIsSettingsOpen(false)}
              >
                Close
              </button>
            </div>

            <section className="settings-section">
              <h2>Background</h2>
              <div className="setting-row">
                <label className="setting-label" htmlFor="background-mode">
                  Background mode
                </label>
                <select
                  id="background-mode"
                  className="setting-control"
                  value={appearance.backgroundMode}
                  onChange={(event) =>
                    updateAppearance(
                      "backgroundMode",
                      event.target.value as BackgroundMode,
                    )
                  }
                >
                  <option value="color">Solid color</option>
                  <option value="image">Background image</option>
                </select>
              </div>

              <div className="setting-row">
                <label className="setting-label" htmlFor="background-color">
                  Background color
                </label>
                <input
                  id="background-color"
                  className="setting-color"
                  type="color"
                  value={appearance.backgroundColor}
                  onChange={(event) =>
                    updateAppearance("backgroundColor", event.target.value)
                  }
                />
              </div>

              <div className="setting-row setting-row-stack">
                <span className="setting-label">Background image</span>
                <div className="background-actions">
                  <button
                    className="toolbar-button"
                    type="button"
                    onClick={() => fileInputRef.current?.click()}
                  >
                    Choose Image
                  </button>
                  <button
                    className="toolbar-button"
                    type="button"
                    onClick={resetBackgroundImage}
                  >
                    Remove Image
                  </button>
                </div>
                <input
                  ref={fileInputRef}
                  className="settings-file-input"
                  type="file"
                  accept="image/*"
                  onChange={handleBackgroundImagePick}
                />
                <div className="setting-help">
                  {appearance.backgroundImage
                    ? "Image selected. Stored locally in app settings."
                    : "No background image selected."}
                </div>
              </div>

              <div className="setting-row">
                <label className="setting-label" htmlFor="overlay-opacity">
                  Image overlay
                </label>
                <div className="setting-inline">
                  <input
                    id="overlay-opacity"
                    className="setting-range"
                    type="range"
                    min="0.15"
                    max="0.85"
                    step="0.05"
                    value={appearance.backgroundOverlayOpacity}
                    onChange={(event) =>
                      updateAppearance(
                        "backgroundOverlayOpacity",
                        Number(event.target.value),
                      )
                    }
                  />
                  <span className="setting-value">
                    {Math.round(appearance.backgroundOverlayOpacity * 100)}%
                  </span>
                </div>
              </div>
            </section>

            <section className="settings-section">
              <h2>Typography</h2>
              <div className="setting-row">
                <label className="setting-label" htmlFor="font-family">
                  Font family
                </label>
                <select
                  id="font-family"
                  className="setting-control"
                  value={appearance.fontFamily}
                  onChange={(event) =>
                    updateAppearance("fontFamily", event.target.value)
                  }
                >
                  {FONT_FAMILIES.map((font) => (
                    <option key={font} value={font}>
                      {font}
                    </option>
                  ))}
                </select>
              </div>

              <div className="setting-row">
                <label className="setting-label" htmlFor="font-size">
                  Font size
                </label>
                <div className="setting-inline">
                  <input
                    id="font-size"
                    className="setting-range"
                    type="range"
                    min="11"
                    max="22"
                    step="1"
                    value={appearance.fontSize}
                    onChange={(event) =>
                      updateAppearance("fontSize", Number(event.target.value))
                    }
                  />
                  <span className="setting-value">{appearance.fontSize}px</span>
                </div>
              </div>

              <div className="setting-row">
                <label className="setting-label" htmlFor="font-weight">
                  Font weight
                </label>
                <select
                  id="font-weight"
                  className="setting-control"
                  value={appearance.fontWeight}
                  onChange={(event) =>
                    updateAppearance("fontWeight", Number(event.target.value))
                  }
                >
                  {FONT_WEIGHTS.map((weight) => (
                    <option key={weight} value={weight}>
                      {weight}
                    </option>
                  ))}
                </select>
              </div>

              <div className="setting-row">
                <label className="setting-label" htmlFor="font-italic">
                  Italic
                </label>
                <label className="setting-toggle">
                  <input
                    id="font-italic"
                    type="checkbox"
                    checked={appearance.italic}
                    onChange={(event) =>
                      updateAppearance("italic", event.target.checked)
                    }
                  />
                  <span>{appearance.italic ? "On" : "Off"}</span>
                </label>
              </div>
            </section>

            <section className="settings-section">
              <h2>Semantic Colors</h2>
              <div className="settings-grid">
                <ColorSetting
                  label="Prompt"
                  value={appearance.promptColor}
                  onChange={(value) => updateAppearance("promptColor", value)}
                />
                <ColorSetting
                  label="Mode tag"
                  value={appearance.modeTagColor}
                  onChange={(value) => updateAppearance("modeTagColor", value)}
                />
                <ColorSetting
                  label="Translation hint"
                  value={appearance.translationHintColor}
                  onChange={(value) =>
                    updateAppearance("translationHintColor", value)
                  }
                />
                <ColorSetting
                  label="Success"
                  value={appearance.successColor}
                  onChange={(value) => updateAppearance("successColor", value)}
                />
                <ColorSetting
                  label="Warning"
                  value={appearance.warningColor}
                  onChange={(value) => updateAppearance("warningColor", value)}
                />
                <ColorSetting
                  label="Error"
                  value={appearance.errorColor}
                  onChange={(value) => updateAppearance("errorColor", value)}
                />
                <ColorSetting
                  label="Stdout"
                  value={appearance.stdoutColor}
                  onChange={(value) => updateAppearance("stdoutColor", value)}
                />
                <ColorSetting
                  label="Stderr"
                  value={appearance.stderrColor}
                  onChange={(value) => updateAppearance("stderrColor", value)}
                />
                <ColorSetting
                  label="Notice"
                  value={appearance.noticeColor}
                  onChange={(value) => updateAppearance("noticeColor", value)}
                />
                <ColorSetting
                  label="Info"
                  value={appearance.infoColor}
                  onChange={(value) => updateAppearance("infoColor", value)}
                />
                <ColorSetting
                  label="Suggestion"
                  value={appearance.suggestionColor}
                  onChange={(value) => updateAppearance("suggestionColor", value)}
                />
              </div>
            </section>

            <section className="settings-section">
              <h2>Terminal Base Theme</h2>
              <div className="settings-grid">
                <ColorSetting
                  label="Terminal foreground"
                  value={appearance.terminalForeground}
                  onChange={(value) =>
                    updateAppearance("terminalForeground", value)
                  }
                />
                <ColorSetting
                  label="Terminal background"
                  value={appearance.terminalBackground}
                  onChange={(value) =>
                    updateAppearance("terminalBackground", value)
                  }
                />
                <ColorSetting
                  label="Cursor"
                  value={appearance.cursorColor}
                  onChange={(value) => updateAppearance("cursorColor", value)}
                />
                <ColorSetting
                  label="Selection"
                  value={appearance.selectionColor}
                  onChange={(value) => updateAppearance("selectionColor", value)}
                />
              </div>
            </section>

            <section className="settings-section settings-footer">
              <button className="toolbar-button" type="button" onClick={resetAppearance}>
                Reset to Default Theme
              </button>
            </section>
          </aside>
        </div>
      ) : null}
    </main>
  );
}

function ColorSetting({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="color-setting">
      <span>{label}</span>
      <input
        className="setting-color"
        type="color"
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}

function buildCatalogSuggestionItemStyle(
  settings: AppearanceSettings,
  suggestion: CatalogSuggestion,
  selected: boolean,
): CSSProperties {
  const borderColor = selected
    ? hexToRgba(settings.modeTagColor, 0.44)
    : hexToRgba(settings.infoColor, 0.14);
  const backgroundColor = selected
    ? hexToRgba(settings.modeTagColor, 0.14)
    : "rgba(5, 13, 22, 0.88)";

  return {
    borderColor,
    backgroundColor,
    color:
      suggestion.risk === "high"
        ? settings.errorColor
        : suggestion.risk === "medium"
          ? settings.warningColor
          : settings.stdoutColor,
  };
}

function formatCatalogSuggestionMeta(suggestion: CatalogSuggestion): string {
  const parts = [
    suggestion.intentId,
    `${displayShellLabel(suggestion.sourceShell)} -> ${displayShellLabel(
      suggestion.targetShell,
    )}`,
    suggestion.description,
  ];

  return parts.filter((value) => value.length > 0).join(" · ");
}

function displayShellLabel(shell: string): string {
  switch (shell) {
    case "windows_cmd":
      return "Windows CMD";
    case "powershell":
      return "PowerShell";
    case "windows":
      return "Windows";
    case "macos":
      return "macOS";
    case "ubuntu":
      return "Ubuntu";
    default:
      return shell;
  }
}

function loadAppearanceSettings(): AppearanceSettings {
  if (typeof window === "undefined") {
    return DEFAULT_APPEARANCE_SETTINGS;
  }

  const saved = window.localStorage.getItem(APPEARANCE_STORAGE_KEY);
  if (!saved) {
    return DEFAULT_APPEARANCE_SETTINGS;
  }

  try {
    const parsed = JSON.parse(saved) as Partial<AppearanceSettings>;
    return {
      ...DEFAULT_APPEARANCE_SETTINGS,
      ...parsed,
      backgroundMode:
        parsed.backgroundMode === "image" ? "image" : "color",
      backgroundImage:
        typeof parsed.backgroundImage === "string"
          ? parsed.backgroundImage
          : null,
      backgroundOverlayOpacity:
        typeof parsed.backgroundOverlayOpacity === "number"
          ? clamp(parsed.backgroundOverlayOpacity, 0.15, 0.85)
          : DEFAULT_APPEARANCE_SETTINGS.backgroundOverlayOpacity,
      fontSize:
        typeof parsed.fontSize === "number"
          ? clamp(parsed.fontSize, 11, 22)
          : DEFAULT_APPEARANCE_SETTINGS.fontSize,
      fontWeight:
        typeof parsed.fontWeight === "number"
          ? parsed.fontWeight
          : typeof parsed.fontWeight === "string"
            ? Number(parsed.fontWeight) || DEFAULT_APPEARANCE_SETTINGS.fontWeight
          : DEFAULT_APPEARANCE_SETTINGS.fontWeight,
      italic: Boolean(parsed.italic),
    };
  } catch {
    return DEFAULT_APPEARANCE_SETTINGS;
  }
}

function loadTranslateHistory(): string[] {
  if (typeof window === "undefined") {
    return [];
  }

  const saved = window.localStorage.getItem(TRANSLATE_HISTORY_STORAGE_KEY);
  if (!saved) {
    return [];
  }

  try {
    const parsed = JSON.parse(saved);
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed
      .filter((entry): entry is string => typeof entry === "string")
      .map((entry) => entry.trim())
      .filter((entry) => entry.length > 0)
      .slice(-TRANSLATE_HISTORY_LIMIT);
  } catch {
    return [];
  }
}

function buildXtermTheme(settings: AppearanceSettings): TerminalThemeLike {
  const background =
    settings.backgroundMode === "image" && settings.backgroundImage
      ? hexToRgba(settings.terminalBackground, settings.backgroundOverlayOpacity)
      : settings.terminalBackground;

  return {
    background,
    foreground: settings.terminalForeground,
    cursor: settings.cursorColor,
    selectionBackground: hexToRgba(settings.selectionColor, 0.32),
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
    white: settings.terminalForeground,
    yellow: "#f3c65f",
  };
}

function buildTranslatePromptSegments(
  currentDir: string,
  homeDir: string | null,
  settings: AppearanceSettings,
): StyledSegment[] {
  return [
    { text: "CLI4ALL ", color: settings.promptColor, bold: true },
    { text: "[translate]", color: settings.modeTagColor, bold: true },
    {
      text: ` ${formatPromptPath(currentDir, homeDir)} `,
      color: settings.promptColor,
      bold: true,
    },
    { text: "❯ ", color: settings.promptColor, bold: true },
  ];
}

function formatPromptPath(currentDir: string, homeDir: string | null): string {
  if (!currentDir) {
    return "~";
  }

  if (!homeDir || homeDir.length === 0) {
    return currentDir;
  }

  if (currentDir === homeDir) {
    return "~";
  }

  if (currentDir.startsWith(`${homeDir}/`)) {
    return `~${currentDir.slice(homeDir.length)}`;
  }

  if (currentDir.startsWith(`${homeDir}\\`)) {
    return `~${currentDir.slice(homeDir.length)}`;
  }

  return currentDir;
}

function shouldPrintCompactTranslationHint(
  response: Pick<
    SubmitTerminalLineResponse,
    | "detectedSource"
    | "matchedIntent"
    | "originalCommand"
    | "translatedCommand"
    | "currentOs"
  >,
): boolean {
  if (!response.translatedCommand) {
    return false;
  }

  if (response.detectedSource === BUILTIN_SOURCE) {
    return !matchesIntent(response.matchedIntent, [
      "print_working_directory",
      "change_directory",
    ]);
  }

  return true;
}

function matchesIntent(intent: string | null, accepted: string[]): boolean {
  return intent !== null && accepted.includes(intent);
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

function detailModeLabel(mode: DetailMode): string {
  return mode === "clean" ? "Clean" : "Verbose";
}

function printModeSwitchNotice(
  terminal: Terminal,
  mode: TerminalMode,
  detailMode: DetailMode,
  settings: AppearanceSettings,
) {
  if (detailMode === "verbose") {
    writeTerminalAndScroll(
      terminal,
      `${styleSegments([
        {
          text: "---------------- CLI4ALL Mode ----------------",
          color: settings.infoColor,
          bold: true,
        },
      ])}\r\n`,
    );
    writeTerminalAndScroll(
      terminal,
      `${styleSegments([
        {
          text: `Switched to ${modeLabel(mode)}`,
          color: settings.infoColor,
          bold: true,
        },
      ])}\r\n`,
    );
    writeTerminalAndScroll(
      terminal,
      `${styleSegments([{ text: MODE_RULE, color: settings.infoColor }])}\r\n`,
    );
    return;
  }

  writeTerminalAndScroll(
    terminal,
    `${styleSegments([
      {
        text: `[CLI4ALL] Switched to ${modeLabel(mode)}`,
        color: settings.infoColor,
        bold: true,
      },
    ])}\r\n`,
  );
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

function buildCompactConfirmationPrompt(riskLevel: string): string {
  return `! Risk: ${riskLevel}. Execute translated command? [y/N]`;
}

function buildTerminalSurfaceStyle(settings: AppearanceSettings): CSSProperties {
  const style: CSSProperties & Record<string, string> = {
    backgroundColor: settings.backgroundColor,
    backgroundImage:
      settings.backgroundMode === "image" && settings.backgroundImage
        ? `url(${settings.backgroundImage})`
        : "none",
    backgroundSize: "cover",
    backgroundPosition: "center",
    backgroundRepeat: "no-repeat",
  };

  style["--terminal-overlay-opacity"] =
    settings.backgroundMode === "image" && settings.backgroundImage
      ? String(settings.backgroundOverlayOpacity)
      : "0";

  return style;
}

function styleSegments(segments: StyledSegment[]): string {
  return segments
    .map((segment) => {
      const codes: string[] = [];
      if (segment.dim) {
        codes.push("2");
      }
      if (segment.bold) {
        codes.push("1");
      }
      if (segment.italic) {
        codes.push("3");
      }
      if (segment.color) {
        const [red, green, blue] = hexToRgb(segment.color);
        codes.push(`38;2;${red};${green};${blue}`);
      }
      if (codes.length === 0) {
        return segment.text;
      }
      return `\u001b[${codes.join(";")}m${segment.text}\u001b[0m`;
    })
    .join("");
}

function pushTranslateHistoryEntry(current: string[], input: string): string[] {
  const normalized = input.trim();
  const lastEntry = current.length > 0 ? current[current.length - 1] : null;
  if (
    normalized.length === 0 ||
    containsSensitiveHistoryData(normalized) ||
    lastEntry === normalized
  ) {
    return current;
  }

  return [...current, normalized].slice(-TRANSLATE_HISTORY_LIMIT);
}

function containsSensitiveHistoryData(input: string): boolean {
  return /(password=|token=|api_key=|secret=|--password\b|--token\b|--api-key\b)/i.test(
    input,
  );
}

function findGhostSuggestion(
  input: string,
  history: string[],
  historyIndex: number | null,
): string {
  if (historyIndex !== null || input.length === 0) {
    return "";
  }

  const inputLower = input.toLowerCase();
  for (let index = history.length - 1; index >= 0; index -= 1) {
    const entry = history[index];
    if (
      entry.length > input.length &&
      entry.toLowerCase().startsWith(inputLower)
    ) {
      return entry.slice(input.length);
    }
  }

  return "";
}

function hexToRgb(color: string): [number, number, number] {
  const normalized = color.replace("#", "");
  const hex =
    normalized.length === 3
      ? normalized
          .split("")
          .map((part) => `${part}${part}`)
          .join("")
      : normalized;

  const value = Number.parseInt(hex, 16);
  return [
    (value >> 16) & 255,
    (value >> 8) & 255,
    value & 255,
  ];
}

function hexToRgba(color: string, alpha: number): string {
  const [red, green, blue] = hexToRgb(color);
  return `rgba(${red}, ${green}, ${blue}, ${clamp(alpha, 0, 1)})`;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}
