// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  type AillyEvent,
  EventKind,
  type FileReference,
  type IndexProgress,
  type IndexStatus,
  type SessionListItem,
  type SourceValue,
  type ToolCall,
} from "../../src/tauri";
import { resetSessionsStore } from "../../src/ui/sessions/store";

let indexed: SessionListItem[] = [];
let eventsBySession: Record<string, AillyEvent[]> = {};
const startRefresh = vi.fn<() => Promise<void>>();
let onProgress: ((progress: IndexProgress) => void) | null = null;
let onComplete: ((status: IndexStatus) => void) | null = null;

vi.mock("../../src/tauri", async (importActual) => {
  const actual = await importActual<typeof import("../../src/tauri")>();
  return {
    ...actual,
    listSessions: async () => indexed,
    getEventPage: async (sessionId: string) => eventsBySession[sessionId] ?? [],
    startRefresh: () => startRefresh(),
    onIndexProgress: async (handler: (progress: IndexProgress) => void) => {
      onProgress = handler;
      return () => {
        onProgress = null;
      };
    },
    onIndexComplete: async (handler: (status: IndexStatus) => void) => {
      onComplete = handler;
      return () => {
        onComplete = null;
      };
    },
  };
});

const SESSION: SessionListItem = {
  id: "claude_code:/home/a.jsonl:one",
  harness: "claude_code",
  project: { Recorded: "/Users/dev/repo" },
  event_count: 6,
  token_total: "Absent",
  recorded_price_micros: "Absent",
  estimated_tokens: "Absent",
  estimated_price_micros: "Absent",
  estimated_as_of: "Absent",
  last_activity: { Recorded: "2026-08-20T15:00:00Z" },
};

/**
 * What the indexer writes into `Event.files`: the two fields `FileReference`
 * already has, plus the two this feature adds — where the claim came from, and
 * why a fragment could not be resolved into a path.
 */
interface FileAccess {
  path: string;
  operation: SourceValue<string>;
  provenance: SourceValue<string>;
  ambiguity: SourceValue<string>;
}

/** A path a dedicated tool field named, so the claim needs no interpretation. */
function fromTool(path: string, operation: string): FileAccess {
  return {
    path,
    operation: { Recorded: operation },
    provenance: { Recorded: "tool" },
    ambiguity: "Absent",
  };
}

/** A path read out of recorded command text — evidence, not a disk fact. */
function fromShell(path: string, operation: string): FileAccess {
  return {
    path,
    operation: { Recorded: operation },
    provenance: { Recorded: "shell" },
    ambiguity: "Absent",
  };
}

/** A command operand the parse refused to resolve, kept with its reason. */
function ambiguous(fragment: string, operation: string, reason: string): FileAccess {
  return {
    path: fragment,
    operation: { Recorded: operation },
    provenance: { Recorded: "shell" },
    ambiguity: { Recorded: reason },
  };
}

const TOOL_READ = "packages/auth/src/session.ts";
const TOOL_WRITE = "packages/auth/src/tokens.ts";
const SHELL_READ = "config/base.yml";
const SHELL_WRITE = "build/out.env";
const SHELL_DELETE = "build/stale.env";
const SCRIPTED_READ = "src/main.rs";
const GLOB_FRAGMENT = "logs/*.txt";

function baseEvent(id: string, ordinal: number, kind: EventKind): AillyEvent {
  return {
    id,
    session_id: SESSION.id,
    kind,
    source: { harness: "claude_code", path: "/home/a.jsonl", line: ordinal, ordinal },
    native_id: "Absent",
    response_id: "Absent",
    model: "Absent",
    timestamp: "Absent",
    turn: "Absent",
    tool_call: "Absent",
    tool_result: "Absent",
    token_usage: "Absent",
    files: "Absent",
    detail: "Absent",
    subagent: "Absent",
  };
}

function toolEvent(
  id: string,
  ordinal: number,
  tool: Partial<ToolCall> & { name: string },
  files: FileAccess[],
): AillyEvent {
  return {
    ...baseEvent(id, ordinal, EventKind.ToolCall),
    tool_call: {
      Recorded: {
        call_id: "Absent",
        input: "Absent",
        command: "Absent",
        path: "Absent",
        url: "Absent",
        cwd: "Absent",
        ...tool,
      } satisfies ToolCall,
    },
    // `FileAccess` is `FileReference` plus this feature's two fields, so the
    // indexed page assigns straight into the existing event shape.
    files: { Recorded: files satisfies FileReference[] },
  };
}

/**
 * One session that exercises every provenance and operation the list has to
 * tell apart: a dedicated `Read` and `Write` for contrast, a shell command that
 * both reads and writes, a shell delete, a reader whose first operand is a
 * script rather than a file, and a glob the parse must not resolve.
 */
const EVENTS: AillyEvent[] = [
  toolEvent("evt-1", 1, { name: "Read", path: { Recorded: TOOL_READ } }, [
    fromTool(TOOL_READ, "read"),
  ]),
  toolEvent("evt-2", 2, { name: "Write", path: { Recorded: TOOL_WRITE } }, [
    fromTool(TOOL_WRITE, "write"),
  ]),
  toolEvent(
    "evt-3",
    3,
    { name: "Bash", command: { Recorded: `cat ${SHELL_READ} >> ${SHELL_WRITE}` } },
    [fromShell(SHELL_READ, "read"), fromShell(SHELL_WRITE, "write")],
  ),
  toolEvent("evt-4", 4, { name: "Bash", command: { Recorded: `rm -f ${SHELL_DELETE}` } }, [
    fromShell(SHELL_DELETE, "delete"),
  ]),
  toolEvent(
    "evt-5",
    5,
    { name: "Bash", command: { Recorded: `sed -n '1,220p' ${SCRIPTED_READ}` } },
    [fromShell(SCRIPTED_READ, "read")],
  ),
  toolEvent("evt-6", 6, { name: "Bash", command: { Recorded: `cat ${GLOB_FRAGMENT}` } }, [
    ambiguous(GLOB_FRAGMENT, "read", "glob not expanded"),
  ]),
];

beforeEach(() => {
  indexed = [SESSION];
  eventsBySession = { [SESSION.id]: EVENTS };
  startRefresh.mockResolvedValue(undefined);
  resetSessionsStore();
});

afterEach(() => {
  cleanup();
  startRefresh.mockReset();
  onProgress = null;
  onComplete = null;
  resetSessionsStore();
});

async function renderApp() {
  const { App } = await import("../../src/App");
  render(<App />);
}

/** The File access row for `path`, as the scope its labels are asserted in. */
function accessRow(list: HTMLElement, path: string): HTMLElement {
  const row = within(list)
    .getAllByRole("listitem")
    .find((item) => item.textContent?.includes(path));
  if (row === undefined) {
    throw new Error(`No File access row for ${path}`);
  }
  return row;
}

describe("Reviewing the files a session's shell commands touched", () => {
  it("lists shell-derived and tool-derived accesses with operation and provenance", async () => {
    await renderApp();

    const summary = await screen.findByRole("region", { name: /session summary/i });
    const fileAccess = within(summary).getByRole("list", { name: /file access/i });

    // A path the harness named in its own field: read, and known to be so
    // because a tool said it, not because anyone read a command.
    const toolRead = accessRow(fileAccess, TOOL_READ);
    expect(toolRead).toHaveTextContent(/read/i);
    expect(toolRead).toHaveTextContent(/tool/i);
    expect(toolRead).not.toHaveTextContent(/shell/i);

    expect(accessRow(fileAccess, TOOL_WRITE)).toHaveTextContent(/write/i);

    // The same two operations, recovered from one recorded command and labelled
    // as evidence about that command rather than as a tool's own claim.
    const shellRead = accessRow(fileAccess, SHELL_READ);
    expect(shellRead).toHaveTextContent(/read/i);
    expect(shellRead).toHaveTextContent(/shell/i);

    const shellWrite = accessRow(fileAccess, SHELL_WRITE);
    expect(shellWrite).toHaveTextContent(/write/i);
    expect(shellWrite).toHaveTextContent(/shell/i);

    // Deletion is its own operation, not a write and not a silent omission.
    const shellDelete = accessRow(fileAccess, SHELL_DELETE);
    expect(shellDelete).toHaveTextContent(/delete/i);
    expect(shellDelete).toHaveTextContent(/shell/i);

    // `sed -n '1,220p' src/main.rs` reads one file. Its script operand is not a
    // second one, however confidently the parse could report it as a path.
    expect(accessRow(fileAccess, SCRIPTED_READ)).toHaveTextContent(/read/i);
    expect(within(fileAccess).queryByText(/1,220p/)).not.toBeInTheDocument();

    // A glob is shown, labelled as unresolved, and never presented as a path.
    const glob = accessRow(fileAccess, GLOB_FRAGMENT);
    expect(glob).toHaveTextContent(/ambiguous/i);
    expect(glob).toHaveTextContent(/glob not expanded/i);

    // Every row is either a path or an ambiguous fragment, and the tile counts
    // only the paths.
    expect(within(fileAccess).getAllByRole("listitem")).toHaveLength(7);
    const tile = within(summary).getByRole("group", { name: /files touched/i });
    expect(within(tile).getByText("6")).toBeInTheDocument();
  });
});
