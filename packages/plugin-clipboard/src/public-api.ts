import { inject, Injectable } from '@angular/core'
import { NativeModules } from '@angular-native/platform'

/**
 * The system clipboard.
 *
 * It is the reference example of how a plugin is written: an Angular service
 * with no logic in it whatsoever —the types out and back and little else— on top
 * of a native module that does have some, written once in Swift and once in
 * Java.
 *
 * On iOS it is `UIPasteboard`; on Android, `ClipboardManager`. Neither of the
 * two can be touched from the engine's thread, so the calls go through the
 * plugin queue and are served on the UI thread. Hence everything returning a
 * promise.
 */
@Injectable({ providedIn: 'root' })
export class Clipboard {
  private readonly modules = inject(NativeModules)

  /** Copies a piece of text. */
  write(text: string): Promise<void> {
    return this.modules.call<void>('clipboard', 'write', { text })
  }

  /**
   * Whatever has been copied, or an empty string if there is no text.
   *
   * On iOS 16 and later, reading something another app copied shows a system
   * notice; reading what the app itself copied does not.
   */
  read(): Promise<string> {
    return this.modules.call<string>('clipboard', 'read')
  }

  /** Whether there is text, without reading it. On iOS this does not trigger the system notice. */
  hasText(): Promise<boolean> {
    return this.modules.call<boolean>('clipboard', 'hasText')
  }
}
