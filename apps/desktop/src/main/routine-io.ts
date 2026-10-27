import { randomUUID } from 'node:crypto'
import { readFile, stat, writeFile } from 'node:fs/promises'
import { roleLabelOf } from '../shared/ipc.js'
import type { PublicTeammate, RoutineExportResponse, RoutineFolderResponse, RoutineImportPreviewResponse, RoutineMutationResponse, RoutineRunResponse, RoutineTemplatesResponse, TeammateRoute } from '../shared/ipc.js'
import { listRoutineTemplates, readRoutineTemplate, templateChangesFiles, templateOnAnotherAgent } from './routine-templates.js'
import { resolveValues } from '../shared/routine-inputs.js'
import type { RoutineValues } from '../shared/routine-inputs.js'
import { absolutePathsIn, MAX_ROUTINE_FILE_BYTES, parseRoutineFile, pathsAsInputs, routineExportText, routineFileName, routineToFile } from './routine-file.js'
import type { RoutineFile } from './routine-file.js'
import type { RoutineStore } from './routine-store.js'

const rejected = (message: string) => ({ ok: false as const, error: { code: 'ROUTINE_REJECTED' as const, message } })
const record = (value: unknown): Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value) ? value as Record<string, unknown> : {}

/** W7: host-owned file dialogs, preview receipts and folder selections. No import starts work. */
export function createRoutineIO(options: {
  readonly routines: RoutineStore
  readonly team: () => Promise<readonly PublicTeammate[]>
  readonly connectors: () => readonly string[]
  readonly workspace: () => string | undefined
  readonly pickFolder: () => Promise<string | undefined>
  readonly pickImport: () => Promise<string | undefined>
  readonly pickExport: (name: string) => Promise<string | undefined>
  readonly run: (id: string, values?: RoutineValues) => Promise<RoutineRunResponse>
  /** Where the starter routines ship (0.615, routine-templates.ts). */
  readonly templates?: string
}) {
  const folders = new Set<string>()
  const previews = new Map<string, { file: RoutineFile; workspace: string | undefined; from: 'file' | 'template' }>()
  /** A file read and checked, held under a receipt for the Import that follows: an imported file and a template alike. */
  const previewOf = (file: RoutineFile, from: 'file' | 'template', changesFiles = false, onAnotherAgent?: readonly boolean[]): RoutineImportPreviewResponse => {
    const token = randomUUID()
    if (previews.size >= 8) previews.delete(previews.keys().next().value!)
    previews.set(token, { file, workspace: options.workspace(), from })
    const names = new Set(options.connectors().map((name) => name.toLowerCase()))
    return { ok: true, data: { preview: {
      token, name: file.name, steps: file.steps, inputs: file.inputs,
      handOffRoles: file.handOffs.map((entry) => entry.role),
      handOffChecks: file.handOffs.map((entry) => entry.check === true),
      ...(changesFiles ? { changesFiles: true as const } : {}),
      ...(onAnotherAgent === undefined ? {} : { handOffOnAnotherAgent: onAnotherAgent }),
      ...(file.route.runtime === undefined ? {} : { runtime: file.route.runtime }),
      connectors: file.connectors.map((name) => ({ name, present: names.has(name.toLowerCase()) }))
    } } }
  }
  return {
    /** The starter routines, in the order they are offered. */
    async templates(): Promise<RoutineTemplatesResponse> {
      if (options.templates === undefined) return { ok: true, data: { templates: [] } }
      const listed = await listRoutineTemplates(options.templates)
      return { ok: true, data: { templates: listed.templates } }
    },
    /** One starter routine, read by the reader an import uses, previewed by its preview. */
    async previewTemplate(id: unknown): Promise<RoutineImportPreviewResponse> {
      if (options.templates === undefined) return rejected('This Locust ships no starter routines.')
      const read = await readRoutineTemplate(options.templates, id)
      return read.ok ? previewOf(read.file, 'template', typeof id === 'string' && templateChangesFiles(id), typeof id === 'string' ? templateOnAnotherAgent(id) : undefined) : rejected(read.message)
    },
    folderWasChosen: (path: string): boolean => folders.has(path),
    async folder(): Promise<RoutineFolderResponse> {
      const path = await options.pickFolder()
      if (path === undefined) return { ok: false, error: { code: 'CANCELLED', message: 'No folder was chosen.' } }
      folders.add(path)
      if (folders.size > 100) folders.delete(folders.values().next().value!)
      return { ok: true, data: { path } }
    },
    async run(id: unknown, given: unknown): Promise<RoutineRunResponse> {
      if (typeof id !== 'string') return rejected('That routine could not be started.')
      const routine = await options.routines.get(id)
      if (routine === undefined) return rejected('That routine no longer exists.')
      const settled = resolveValues(routine.inputs ?? [], given)
      if (!settled.ok) return rejected(settled.message)
      for (const input of routine.inputs ?? []) {
        const path = settled.values[input.key]
        if (input.kind === 'folder' && path && !folders.has(path)) return rejected(`Choose "${input.label}" with the folder picker before running.`)
      }
      return options.run(id, given === undefined ? undefined : settled.values)
    },
    async export(raw: unknown): Promise<RoutineExportResponse> {
      const request = record(raw)
      if (typeof request.routineId !== 'string' || (request.paths !== undefined && request.paths !== 'input' && request.paths !== 'keep')) return rejected('That export request could not be read.')
      const routine = await options.routines.get(request.routineId)
      if (routine === undefined) return rejected('That routine no longer exists.')
      const flagged = absolutePathsIn(routine.steps)
      if (flagged.length > 0 && request.paths === undefined) return { ok: false, error: { code: 'ABSOLUTE_PATHS', message: 'These steps name paths on this machine. Make them inputs to enter on another machine, or keep them in the file.', flagged } }
      const using = request.paths === 'input' ? pathsAsInputs(routine.steps, routine.inputs ?? []) : { ok: true as const, steps: routine.steps, inputs: routine.inputs ?? [] }
      if (!using.ok) return rejected(using.message)
      const team = await options.team()
      const file = routineToFile(routine, { ...using, connectorNames: options.connectors(), roleOf: (id) => {
        const mate = team.find((entry) => entry.teammateId === id)
        return mate === undefined ? undefined : roleLabelOf(mate)
      } })
      // Markdown a person can read and edit (0.834), or the JSON file when the Markdown would not carry it exactly.
      const { text, markdown } = routineExportText(file)
      const checked = parseRoutineFile(text)
      if (!checked.ok) return rejected(checked.message)
      const path = await options.pickExport(routineFileName(routine.name, markdown))
      if (path === undefined) return { ok: true, data: { cancelled: true } }
      await writeFile(path, text, 'utf8')
      return { ok: true, data: { path } }
    },
    async preview(): Promise<RoutineImportPreviewResponse> {
      const path = await options.pickImport()
      if (path === undefined) return { ok: true, data: {} }
      if ((await stat(path)).size > MAX_ROUTINE_FILE_BYTES) return rejected('That file is too large to be a routine.')
      const parsed = parseRoutineFile(await readFile(path, 'utf8'))
      if (!parsed.ok) return rejected(parsed.message)
      return previewOf(parsed.file, 'file')
    },
    async import(raw: unknown): Promise<RoutineMutationResponse> {
      const request = record(raw)
      const held = typeof request.token === 'string' ? previews.get(request.token) : undefined
      if (held === undefined) return rejected('Open the routine again before importing it.')
      // A routine runs in the folder it is added in, so a template needs one open as a file does.
      if (held.workspace !== options.workspace() || held.workspace === undefined) return rejected(held.from === 'template' ? 'Choose the folder your teammates work in first: a routine runs in the folder it is added in.' : 'Open the project folder and preview the file there before importing it.')
      const mate = (await options.team()).find((entry) => entry.teammateId === request.teammateId)
      if (mate === undefined) return rejected('Choose a teammate who is still on this team.')
      // Another request may have consumed this receipt while the roster was read.
      if (previews.get(request.token as string) !== held) return rejected(held.from === 'template' ? 'This template was already added. Choose it again to add another copy.' : 'This preview was already imported. Open the routine file again to import another copy.')
      if (held.workspace !== options.workspace()) return rejected(held.from === 'template' ? 'The project folder changed. Choose the template again in the folder it is for.' : 'The project folder changed. Preview the file in the folder where it belongs.')
      // The selected teammate receives every step; source roles are hints in the preview.
      const base = mate.route ?? request.route as TeammateRoute
      const route = { ...base, mode: 'ask' as const }
      previews.delete(request.token as string)
      const routine = await options.routines.create({ name: held.file.name, steps: held.file.steps, inputs: held.file.inputs,
        teammateId: mate.teammateId, route, learnedFrom: [], workspaceId: held.workspace, ...(held.file.schedule === undefined ? {} : { schedule: held.file.schedule }) })
      return { ok: true, data: { routine } }
    }
  }
}
