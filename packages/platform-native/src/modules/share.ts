import { inject, Injectable } from '@angular/core'

import { NativeModules, type NativePlatform } from '../native-modules'

/** What is being handed over. At least one of the four has to be there. */
export interface ShareRequest {
  /** A line of text. On Android it and `url` end up in the same body. */
  text?: string
  /** A link. It travels as a URL, so a receiving app can treat it as one. */
  url?: string
  /**
   * The subject, where the receiving app has one: the mail subject on Apple's
   * platforms, `EXTRA_SUBJECT` on Android, and the chooser's title.
   */
  title?: string
  /**
   * Files, by the path `files` gave. On Android they leave the app through a
   * `FileProvider`, so they have to be inside `files.documentsDirectory()` or
   * `files.cacheDirectory()` — or be a `content://` path that came back from
   * `files.pick()`.
   */
  files?: string[]
}

/**
 * Where a share sheet exists.
 *
 * Not on a television —`UIActivityViewController` does not exist on tvOS, and
 * there is no AirDrop and nothing to hand anything to— and not on a watch, on
 * either platform: watchOS has no sheet at all, and on Wear OS nothing declares
 * `ACTION_SEND`, so the chooser would come up empty. `canShare()` answers
 * `false` there, so a button can be hidden rather than found out about by being
 * rejected.
 */
export type SharePlatform = Exclude<NativePlatform, 'tvos' | 'watchos' | 'wearos'>

/**
 * The system share sheet.
 *
 * `UIActivityViewController` on the phone, the iPad and the headset,
 * `NSSharingServicePicker` on the Mac, `Intent.ACTION_SEND` inside a chooser on
 * Android. It is deliberately the platform's own and not a screen of ours that
 * looks like one: what appears in that sheet is every app on the device that
 * said it can receive this kind of thing, plus AirDrop, plus whatever the person
 * pinned — and none of that is reproducible.
 */
@Injectable({ providedIn: 'root' })
export class Share {
  private readonly modules = inject(NativeModules)

  /**
   * Whether this platform has a sheet at all. Ask before showing the button;
   * `share()` rejects by name on the platforms where the answer is `false`.
   */
  canShare(): Promise<boolean> {
    return this.modules.call<boolean>('share', 'canShare')
  }

  /**
   * Opens the sheet and waits.
   *
   * `false` means the person closed it without sharing, which is not a failure
   * and does not reject.
   *
   * On Android it always answers `true` once the chooser is up, and that is not
   * carelessness: Android tells the app which component was picked only through
   * a broadcast receiver registered beforehand, and never whether the person
   * went through with it. Answering what is actually known beats inventing the
   * rest.
   */
  share(request: ShareRequest): Promise<boolean> {
    return this.modules.call<boolean>('share', 'share', request)
  }
}
