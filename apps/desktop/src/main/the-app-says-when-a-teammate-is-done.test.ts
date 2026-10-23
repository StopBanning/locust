import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

/**
 * KNOW WHEN A TEAMMATE IS DONE (0.379), the wiring between the pieces.
 *
 * Each piece is tested on its own -- keep-awake.ts, taskbar-attention.ts,
 * attention.ts (runFinished), finishedToast.ts. What a unit test cannot hold
 * is that the host, the preload and the window are joined up; a drive
 * cannot show a toast (mayShowToasts refuses them to scripted launches), so
 * the joins are pinned here, at the source.
 */
const read = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')
const main = read('./index.ts')
const preload = read('../shared/desktop-api.ts')
const app = read('../renderer/src/App.tsx')

describe('the app says when a teammate is done', () => {
  it('holds the SYSTEM awake while any run is live, in either transport -- never the display', () => {
    expect(main).toContain("start: () => powerSaveBlocker.start('prevent-app-suspension')")
    expect(main).toContain('keepAwake.update(codexMissions.liveMissionIds().length + antigravityMissions.liveMissionIds().length)')
    expect(main).not.toContain("'prevent-display-sleep'")
  })

  it('puts the needs-you count on the taskbar: the dot, and a flash when it rises', () => {
    expect(app).toContain('window.desktop?.setNeedsYouCount?.(needsYouCount)')
    expect(preload).toContain('setNeedsYouCount: (count) => ipcRenderer.send(NEEDS_YOU_COUNT_CHANNEL, count)')
    expect(main).toContain('const count = needsYouCountFrom(raw)')
    expect(main).toContain("window.setOverlayIcon(said.overlay === 'dot' ? attentionDotImage : null")
    expect(main).toContain('if (said.flash) {')
  })

  it('tells a long finish once, from the moment a live run becomes finished, and a click opens it', () => {
    // Only on the transition, and through the rule that decides it is worth telling.
    expect(app).toContain("const wasLive = before === 'starting' || before === 'running' || before === 'cancelling'")
    expect(app).toContain('const toast = finishedToast({')
    expect(app).toContain('window.desktop?.notifyFinished?.(')
    expect(preload).toContain('notifyFinished: (finish) => ipcRenderer.send(RUN_FINISHED_CHANNEL, finish)')
    // The host believes only a bounded finish, and says it through attention.
    expect(main).toContain('const finish = finishFrom(raw)')
    expect(main).toContain('attention.runFinished(finish)')
    // Clicked, the toast opens the conversation.
    expect(main).toContain('target.webContents.send(ATTENTION_OPEN_MISSION_CHANNEL, missionId)')
    expect(app).toContain('window.desktop?.onAttentionOpenMission?.((missionId) => openMissionRef.current(missionId))')
  })
})
