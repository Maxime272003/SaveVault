import { invoke } from "@tauri-apps/api/core";

export type Store = "steam" | "epic" | "ubisoft" | "gog" | "other";

export interface Location {
  path: string;
  kind: "dir" | "files";
  files: string[];
  size: number;
  fileCount: number;
  modified: number | null;
  custom: boolean;
}

export interface Game {
  id: string;
  title: string;
  source: Store;
  steamAppId: number | null;
  installed: boolean;
  installDir: string | null;
  cloudSteam: boolean;
  locations: Location[];
  size: number;
  fileCount: number;
  modified: number | null;
  accounts: string[];
}

export interface UnknownDir {
  path: string;
  name: string;
  hint: string | null;
  steamAppId: number | null;
  suggestion: string | null;
  size: number;
  fileCount: number;
  modified: number | null;
}

export interface ScanResult {
  games: Game[];
  unidentified: UnknownDir[];
  scannedAt: number;
  accounts: string[];
}

export type BackupKind = "manual" | "pre-restore" | "pre-delete";

export interface BackupInfo {
  path: string;
  fileName: string;
  game: string;
  createdAt: number;
  size: number;
  kind: BackupKind;
}

export interface CustomEntry {
  game: string;
  path: string;
}

export interface AppConfig {
  backupDir: string | null;
  retentionEnabled: boolean;
  retentionCount: number;
  language: "fr" | "en";
  steamgriddbKey: string;
  checkUpdatesOnStart: boolean;
  customEntries: CustomEntry[];
  ignoredPaths: string[];
}

export interface ManifestStatus {
  loaded: boolean;
  gameCount: number;
  updatedAt: number | null;
}

export interface Cover {
  path: string;
  portrait: boolean;
}

export interface CacheStats {
  count: number;
  bytes: number;
}

export interface TitleHit {
  name: string;
  steamId: number | null;
}

export const api = {
  getConfig: () => invoke<AppConfig>("get_config"),
  saveConfig: (config: AppConfig) => invoke<void>("save_config", { config }),
  getManifestStatus: () => invoke<ManifestStatus>("get_manifest_status"),
  updateManifest: (force = false) => invoke<ManifestStatus>("update_manifest", { force }),
  getCachedScan: () => invoke<ScanResult | null>("get_cached_scan"),
  runScan: () => invoke<ScanResult>("run_scan"),
  createBackup: (gameId: string) => invoke<BackupInfo>("create_backup", { gameId }),
  backupUnknown: (path: string) => invoke<BackupInfo>("backup_unknown", { path }),
  deleteUnknown: (path: string) => invoke<void>("delete_unknown", { path }),
  listBackups: () => invoke<BackupInfo[]>("list_backups"),
  restoreBackup: (path: string) => invoke<void>("restore_backup", { path }),
  deleteBackup: (path: string) => invoke<void>("delete_backup", { path }),
  deleteGameSaves: (gameId: string) => invoke<void>("delete_game_saves", { gameId }),
  isGameRunning: (gameId: string) => invoke<boolean | null>("is_game_running", { gameId }),
  openPath: (path: string) => invoke<void>("open_path", { path }),
  getCover: (appId: number | null, title: string) => invoke<Cover | null>("get_cover", { appId, title }),
  coverCacheStats: () => invoke<CacheStats>("cover_cache_stats"),
  clearCoverCache: () => invoke<void>("clear_cover_cache"),
  searchTitles: (query: string) => invoke<TitleHit[]>("search_titles", { query }),
};
