import { api } from "./api";

export interface CoverImage {
  src: string;
  portrait: boolean;
}

const pending = new Map<string, Promise<CoverImage | null>>();
/** Résultats déjà connus, lisibles sans attendre (évite tout clignotement au re-rendu). */
const resolved = new Map<string, CoverImage | null>();
const queue: (() => void)[] = [];
let active = 0;
const MAX_PARALLEL = 6;

function next() {
  while (active < MAX_PARALLEL && queue.length) {
    active++;
    queue.shift()!();
  }
}

const keyOf = (appId: number | null, title: string) => (appId != null ? `s${appId}` : `t${title.toLowerCase()}`);

/** Jaquette déjà chargée : `undefined` si elle ne l'a jamais été, `null` s'il n'y en a pas. */
export function peekCover(appId: number | null, title: string): CoverImage | null | undefined {
  return resolved.get(keyOf(appId, title));
}

/** Jaquette d'un jeu, chargée au plus 6 à la fois et mémorisée pour la session. */
export function loadCover(appId: number | null, title: string): Promise<CoverImage | null> {
  const key = keyOf(appId, title);
  let p = pending.get(key);
  if (!p) {
    p = new Promise<CoverImage | null>((resolve) => {
      queue.push(async () => {
        let c: CoverImage | null = null;
        try {
          c = await api.getCover(appId, title);
        } catch {
          c = null;
        } finally {
          resolved.set(key, c);
          resolve(c);
          active--;
          next();
        }
      });
      next();
    });
    pending.set(key, p);
  }
  return p;
}

export function resetCoverCache() {
  pending.clear();
  resolved.clear();
}
