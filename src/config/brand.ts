export const APP_NAME = "CC MiniRoute";
export const REPOSITORY_OWNER = "strmforge";
export const REPOSITORY_NAME = "CC-MiniRoute";
export const REPOSITORY_URL = `https://github.com/${REPOSITORY_OWNER}/${REPOSITORY_NAME}`;
export const PROJECT_HOME_URL = REPOSITORY_URL;
export const RELEASES_URL = `${REPOSITORY_URL}/releases`;
export const DEFAULT_PROXY_PORT = 15731;

export function releaseTagUrl(version: string) {
  return `${RELEASES_URL}/tag/${version}`;
}
