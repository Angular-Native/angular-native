import { inject, Injectable } from '@angular/core'
import { NativeModules } from '@angular-native/platform'

/** What the camera read. */
export interface Barcode {
  /** The decoded contents. */
  value: string
  /**
   * The symbology, as the platform names it: `org.iso.QRCode`, `org.gs1.EAN-13`,
   * `org.iso.Code128` and so on. It is passed through rather than translated
   * into a vocabulary of ours — there are dozens, and a translation table is a
   * thing to fall behind.
   */
  format: string
}

export interface ScanOptions {
  /**
   * Which symbologies to look for. Everything the platform supports by default.
   *
   * Narrowing it is worth doing: a scanner told to look for one format finds it
   * faster and cannot come back with the wrong thing from a busy shelf.
   */
  formats?: string[]
  /** What to put above the viewfinder. */
  prompt?: string
}

/**
 * Reading a barcode with the camera.
 *
 * It opens **full screen** and closes when something is read or the person
 * cancels. There is no inline preview, and that is not an omission: a preview is
 * a view, and a plugin contributes methods rather than views — the boundary is
 * described in [Plugins](/extending/plugins/). A scanner embedded in your own
 * layout would need the core to grow a primitive for it.
 *
 * ```ts
 * const barcode = inject(BarcodeScanner)
 * try {
 *   const { value } = await barcode.scan({ formats: ['org.iso.QRCode'] })
 *   this.open(value)
 * } catch {
 *   // Cancelling rejects, and says it was cancelled.
 * }
 * ```
 *
 * **Not on Android.** Android ships no barcode API. The usual answer is ML Kit,
 * which lives in Play Services and is a Gradle dependency; this toolchain has no
 * Gradle and will not make an app that cannot be installed on a device without
 * Play. The plugin says so at build time rather than at the first scan.
 */
@Injectable({ providedIn: 'root' })
export class BarcodeScanner {
  private readonly modules = inject(NativeModules)

  /** Whether this device can scan at all — a camera, and permission for it. */
  available(): Promise<boolean> {
    return this.modules.call<boolean>('barcode', 'available')
  }

  /**
   * Opens the scanner and resolves with the first thing it reads.
   *
   * Cancelling rejects with a message that says so, so an app that only catches
   * can still tell "they changed their mind" from "the camera is broken".
   */
  scan(options: ScanOptions = {}): Promise<Barcode> {
    return this.modules.call<Barcode>('barcode', 'scan', options)
  }
}
