import { check, type Update } from "@tauri-apps/plugin-updater";
import { api, type AppConfig, type BackupInfo, type Game, type ManifestStatus, type ScanResult, type Store } from "./api";
import { errorText, i18n, t } from "./i18n.svelte";

export type View = "library" | "unidentified" | "backups" | "settings";
export type SortKey = "modified" | "name" | "size";

export interface Toast {
  id: number;
  text: string;
  kind: "info" | "error";
}

class AppStore {
  config = $state<AppConfig | null>(null);
  scan = $state<ScanResult | null>(null);
  scanning = $state(false);
  backups = $state<BackupInfo[]>([]);
  manifest = $state<ManifestStatus | null>(null);
  view = $state<View>("library");
  selectedId = $state<string | null>(null);
  toasts = $state<Toast[]>([]);

  update = $state<Update | null>(null);
  checkingUpdate = $state(false);

  search = $state("");
  sort = $state<SortKey>("modified");
  sources = $state<Record<Store, boolean>>({ steam: true, epic: true, ubisoft: true, gog: true, other: true });
  showInstalled = $state(true);
  showNotInstalled = $state(true);
  account = $state<string>("");

  #toastId = 0;

  get games(): Game[] {
    return this.scan?.games ?? [];
  }

  get selected(): Game | null {
    return this.games.find((g) => g.id === this.selectedId) ?? null;
  }

  get filtered(): Game[] {
    const q = this.search.trim().toLowerCase();
    const list = this.games.filter(
      (g) =>
        this.sources[g.source] &&
        (g.installed ? this.showInstalled : this.showNotInstalled) &&
        (!this.account || g.accounts.includes(this.account)) &&
        (!q || g.title.toLowerCase().includes(q)),
    );
    const by: Record<SortKey, (a: Game, b: Game) => number> = {
      modified: (a, b) => (b.modified ?? 0) - (a.modified ?? 0),
      name: (a, b) => a.title.localeCompare(b.title, i18n.lang),
      size: (a, b) => b.size - a.size,
    };
    return list.sort(by[this.sort]);
  }

  get totalSize(): number {
    return this.games.reduce((sum, g) => sum + g.size, 0);
  }

  backupsFor(title: string): BackupInfo[] {
    return this.backups.filter((b) => b.game === title);
  }

  toast(text: string, kind: Toast["kind"] = "info") {
    const id = ++this.#toastId;
    this.toasts.push({ id, text, kind });
    setTimeout(() => (this.toasts = this.toasts.filter((x) => x.id !== id)), kind === "error" ? 7000 : 4000);
  }

  fail(e: unknown) {
    this.toast(errorText(e), "error");
  }

  async init() {
    this.config = await api.getConfig();
    i18n.lang = this.config.language;
    this.scan = await api.getCachedScan();
    this.selectedId = this.scan?.games[0]?.id ?? null;
    this.manifest = await api.getManifestStatus();
    void this.refreshBackups();
    void this.rescan();
    if (this.config.checkUpdatesOnStart) void this.checkUpdate(true);
  }

  async saveConfig(patch: Partial<AppConfig>, notify = false) {
    if (!this.config) return;
    const next = { ...this.config, ...patch };
    try {
      await api.saveConfig(next);
      this.config = next;
      i18n.lang = next.language;
      if (notify) this.toast(t("settings.saved"));
    } catch (e) {
      this.fail(e);
    }
  }

  async rescan() {
    if (this.scanning) return;
    this.scanning = true;
    try {
      this.scan = await api.runScan();
      if (!this.selected) this.selectedId = this.scan.games[0]?.id ?? null;
      this.manifest = await api.getManifestStatus();
    } catch (e) {
      this.fail(e);
    } finally {
      this.scanning = false;
    }
  }

  async refreshBackups() {
    try {
      this.backups = await api.listBackups();
    } catch (e) {
      this.fail(e);
    }
  }

  async checkUpdate(silent = false) {
    this.checkingUpdate = true;
    try {
      this.update = await check();
      if (!this.update && !silent) this.toast(t("settings.upToDate"));
    } catch (e) {
      if (!silent) this.toast(t("settings.updateError", { e: errorText(e) }), "error");
    } finally {
      this.checkingUpdate = false;
    }
  }
}

export const app = new AppStore();
