/**
 * Deep links: a URL from outside the app, turned into a route.
 *
 * The shell hands the URL to the core, the core keeps it until somebody is
 * listening, and this is the listening end. `NativePlatformLocation` is the only
 * caller: a deep link is a navigation, and navigations go through the history
 * stack like any other.
 */

declare const __an_native: {
  on(module: string, event: string, handler: (payload: unknown) => void): () => void
  deepLinks?: () => string[]
}

/**
 * The module the core emits under. Its twin is `DEEP_LINK_MODULE` in
 * `crates/an-bridge/src/deeplink.rs`; the two cannot share a constant and
 * `scripts/check-deep-links.sh` reads both, because a name that drifts here
 * produces silence rather than an error.
 */
export const DEEP_LINK_MODULE = 'deeplink'

/** The event a link arrives as once the app is running. */
export const DEEP_LINK_EVENT = 'url'

/**
 * The route a URL is asking for.
 *
 * Two shapes reach an app and they are not read the same way:
 *
 * - A **custom scheme** — `myapp://ship/2`. There is no site here: `ship` looks
 *   like a host to a URL parser, but nobody writing `myapp://ship/2` means the
 *   route `/2` on the host `ship`. Everything after the scheme is the path.
 * - A **universal or app link** — `https://example.com/ship/2`. Here the
 *   authority really is a site, it is not part of the route, and what the router
 *   wants is `/ship/2`.
 *
 * Anything that is not a URL at all comes back as `null` rather than as `/`:
 * navigating home because a link was unreadable is worse than ignoring it.
 */
export function routeFromUrl(url: string): string | null {
  const trimmed = url.trim()
  if (!trimmed) return null

  const colon = trimmed.indexOf(':')
  if (colon <= 0) {
    // No scheme. Nothing legitimate arrives this way, so it is not guessed at.
    return null
  }
  const scheme = trimmed.slice(0, colon).toLowerCase()
  let rest = trimmed.slice(colon + 1)
  if (rest.startsWith('//')) {
    rest = rest.slice(2)
    if (scheme === 'http' || scheme === 'https') {
      const authorityEnds = rest.search(/[/?#]/)
      rest = authorityEnds === -1 ? '' : rest.slice(authorityEnds)
    }
  }
  if (!rest) return '/'
  if (rest.startsWith('?') || rest.startsWith('#')) return `/${rest}`
  return rest.startsWith('/') ? rest : `/${rest}`
}

/**
 * What the app was opened with, if anything, and never more than once.
 *
 * It is synchronous, and that is the whole point: on a cold start the URL is
 * already in the core before this bundle is evaluated, and the router has to
 * know where it is going *before* it puts the first screen up. Asking through a
 * native call would answer a frame too late and the app would flash its home
 * screen on the way to the link.
 *
 * Only the last one is returned: if two links piled up before the app could
 * run, the earlier one was superseded before it was ever on screen.
 */
export function takeInitialDeepLink(): string | null {
  if (typeof __an_native === 'undefined' || typeof __an_native.deepLinks !== 'function') {
    return null
  }
  const waiting = __an_native.deepLinks()
  return waiting.length > 0 ? waiting[waiting.length - 1] : null
}

/**
 * Links that arrive while the app is already on screen.
 *
 * Subscribe *before* calling `takeInitialDeepLink()`: taking the queue is what
 * switches the core over from queueing to emitting, and a link landing in
 * between would otherwise have nowhere to go.
 */
export function onDeepLink(handler: (url: string) => void): () => void {
  if (typeof __an_native === 'undefined') return () => {}
  return __an_native.on(DEEP_LINK_MODULE, DEEP_LINK_EVENT, (payload) => {
    const url = (payload as { url?: unknown } | null)?.url
    if (typeof url === 'string') handler(url)
  })
}
