/** A process, as the table holds it: its parent, its name, its command line. */
export interface GuardedProcess {
  readonly ppid: number
  readonly name: string
  readonly line?: string
  /** When it started, ms since 1970 (0 when unknown). */
  readonly created?: number
}

/** The process table, and where this run sits in it (locust-command-guard.mjs). */
export interface ProcessLook {
  readonly table: ReadonlyMap<number, GuardedProcess>
  /** Locust: whatever any teammate started descends from it. */
  readonly root: number
  /** The agent's process Locust started for this run. */
  readonly agent?: number
  /** The hook's ancestors, from its shell up. */
  readonly above: readonly number[]
}

/** Why a command is refused, in the words the model reads, or undefined when it may run; `folders` are where it runs (0.828). */
export function stoppedBecause(command: unknown, look?: () => ProcessLook | undefined, folders?: readonly unknown[]): string | undefined
/** Whether any of these folders is inside a Scout fix's copy (0.828). */
export function inFixCopy(folders: readonly unknown[]): boolean
/** Whether a command would push, add a remote or use gh (0.828). */
export function publishesFromCopy(command: unknown): boolean
/** The process ids a command ends by id, as written in it. */
export function endedIds(command: unknown): number[]
/** The real process table, or undefined when it cannot be read. */
export function lookAtProcesses(): ProcessLook | undefined
