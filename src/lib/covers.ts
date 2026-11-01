import { api } from "./api";

export interface CoverImage {
  src: string;
  portrait: boolean;
}

const cache = new Map<string, Promise<CoverImage | null>>();
const queue: (() => void)[] = [];
let active = 0;
const MAX_PARALLEL = 6;

function next() {
  while (active < MAX_PARALLEL && queue.length) {
    active++;
    queue.shift()!();
  }
}

/** Jaquette d'un jeu, chargée au plus 6 à la fois et mémorisée pour la session. */
export function loadCover(appId: number | null, title: string): Promise<CoverImage | null> {
  const key = appId != null ? `s${appId}` : `t${title.toLowerCase()}`;
  let p = cache.get(key);
  if (!p) {
    p = new Promise<CoverImage | null>((resolve) => {
      queue.push(async () => {
        try {
          const c = await api.getCover(appId, title);
          resolve(c);
        } catch {
          resolve(null);
        } finally {
          active--;
          next();
        }
      });
      next();
    });
    cache.set(key, p);
  }
  return p;
}

export function resetCoverCache() {
  cache.clear();
}
