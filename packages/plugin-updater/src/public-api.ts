import { inject, Injectable } from '@angular/core'
import { NativeModules } from '@angular-native/platform'

/** What is running right now. */
export interface InstalledBundle {
  /** The version you gave `install`, or `packaged` for the one inside the app. */
  version: string
  /**
   * `packaged` — what the store installed.
   * `fresh` — downloaded, will be tried on the next launch.
   * `pending` — running its one trial launch, not yet confirmed.
   * `good` — confirmed by `notifyReady()` and used from now on.
   */
  status: 'packaged' | 'fresh' | 'pending' | 'good'
}

export interface DownloadRequest {
  /** Where the bundle is. Must be `https`. */
  url: string
  /** Yours. It is what `current()` reports and what your server compares against. */
  version: string
  /**
   * Lowercase hex SHA-256 of the file. **Give it.** Without it the app runs
   * whatever answered that URL, which is a remote code execution hole wearing a
   * feature's clothes.
   */
  sha256?: string
}

/**
 * Shipping new JavaScript without going through a store.
 *
 * The app is a native binary plus a `main.js`. The binary can only change
 * through the store; the JavaScript is a file, and this replaces it. That is
 * the whole idea, and it is the same one CodePush and Capgo are built on.
 *
 * **What you may not do with it.** Both stores allow an app to update its own
 * scripts and both forbid using that to change what the app *is*: new features
 * that were never reviewed, a different purpose, anything that would have
 * changed the review. Bug fixes and content are what this is for.
 *
 * ## A bad update costs one launch, not the app
 *
 * An installed bundle is on probation. It runs once; if your code reaches a
 * point where it is plainly working and calls `notifyReady()`, it is kept. If
 * the app starts again and finds a bundle that never confirmed — because it
 * threw on load, or hung — that bundle is thrown away and the one inside the
 * `.app` runs instead.
 *
 * So `notifyReady()` is not a formality. **An update that never calls it is
 * uninstalled on the next launch**, which is exactly what you want from a
 * bundle that crashes, and exactly what you do not want from a working one.
 *
 * ```ts
 * const updater = inject(Updater)
 *
 * // Once the app is up and clearly fine.
 * await updater.notifyReady()
 *
 * const latest = await fetch('https://example.com/bundle.json').then((r) => r.json())
 * if (latest.version !== (await updater.current()).version) {
 *   await updater.download({ url: latest.url, version: latest.version, sha256: latest.sha256 })
 *   // It runs the next time the app is started.
 * }
 * ```
 */
@Injectable({ providedIn: 'root' })
export class Updater {
  private readonly modules = inject(NativeModules)

  /** What is installed, and what state it is in. */
  current(): Promise<InstalledBundle> {
    return this.modules.call<InstalledBundle>('updater', 'current')
  }

  /**
   * Says this bundle works, so it is kept.
   *
   * Call it once the app has actually started — after the first screen, not in
   * a module's constructor. The point is to prove the app got somewhere, and
   * code that runs before anything could go wrong proves nothing.
   *
   * Calling it when the packaged bundle is running does nothing and is not an
   * error, so it does not need guarding.
   */
  notifyReady(): Promise<void> {
    return this.modules.call<void>('updater', 'notifyReady')
  }

  /**
   * Downloads a bundle and installs it for the next launch.
   *
   * It does not swap the running one: replacing the JavaScript under a live app
   * would leave native views on screen belonging to a tree nothing remembers
   * building. The next start picks it up.
   *
   * The download rejects rather than installing anything if the URL is not
   * `https`, or if a `sha256` was given and does not match.
   */
  download(request: DownloadRequest): Promise<void> {
    return this.modules.call<void>('updater', 'download', request)
  }

  /** Throws away whatever is installed and goes back to the packaged bundle on the next launch. */
  reset(): Promise<void> {
    return this.modules.call<void>('updater', 'reset')
  }
}
