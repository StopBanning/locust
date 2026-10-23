import { contextBridge, ipcRenderer } from 'electron'

import { themeFromArguments } from '../shared/appearance.js'
import { desktopApiOver } from '../shared/desktop-api.js'

export type {
  CodexMissionCancelRequest,
  CodexMissionCancelResponse,
  CodexMissionStartRequest,
  CodexMissionStartResponse,
  CodexMissionUpdate,
  DesktopApi,
  LocalRuntimeId,
  MissionHistoryResponse,
  PublicRecoveredMission,
  PublicRuntimeStatus,
  MissionApprovalDecision,
  MissionApprovalKind,
  MissionApprovalRequest,
  MissionMode,
  ModelCatalogResponse,
  PublicRuntimeArtifact,
  PublicModel,
  WorkspaceSettings,
  WorkspaceChooseResponse,
  PublicTeammate,
  TeammateCreateRequest,
  TeammateHue,
  TeammateListResponse,
  TeammateMutationResponse,
  TeammateRole,
  RuntimeAuthState,
  RuntimeDiscoveryResponse,
  RuntimeProbeStatus
} from '../shared/ipc.js'

// The window's calls, over Electron's own ipcRenderer (shared/desktop-api.ts; Locust in a browser builds the same).
contextBridge.exposeInMainWorld('desktop', desktopApiOver(ipcRenderer, { platform: process.platform, initialTheme: themeFromArguments(process.argv) }))
