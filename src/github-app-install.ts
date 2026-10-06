// First-run onboarding step shared by the connect and clone wizards: a GitHub
// App user token can't do anything useful until the app is installed on the
// account, so right after sign-in open the install page and wait for the
// installation to appear instead of failing at a later step.
import { openInBrowser } from "./open-external";
import { checkGithubAppInstalled } from "./vault-api";

const POLL_INTERVAL_MS = 3000;
const GIVE_UP_AFTER_MS = 10 * 60 * 1000;

export async function ensureGithubAppInstalled(
  accessToken: string,
  setStatus: (status: string) => void,
  stale: () => boolean,
): Promise<void> {
  let installation = await checkGithubAppInstalled(accessToken);
  if (installation.status === "installed") return;

  const installUrl = installation.installUrl;
  setStatus(
    `Cerebrite needs to be installed on your GitHub account. Opening ${installUrl} -- choose "All repositories" and install; this continues automatically.`,
  );
  await openInBrowser(installUrl);

  const deadline = Date.now() + GIVE_UP_AFTER_MS;
  while (installation.status === "notInstalled") {
    if (Date.now() >= deadline) {
      throw new Error(`Cerebrite wasn't installed in time. Install it at ${installUrl}, then try again.`);
    }
    await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS));
    if (stale()) return;
    installation = await checkGithubAppInstalled(accessToken);
  }
}
