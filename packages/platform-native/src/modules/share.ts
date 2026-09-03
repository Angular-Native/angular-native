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
 * Not on a television: `UIActivityViewController` does not exist on tvOS, there
 * is no AirDrop and there is nothing to hand anything to. Not on watchOS
 * either, which has no sheet at all.
 *
 * Wear OS is in, and it is the one that has to be checked rather than assumed.
 * A Wear watch does resolve `ACTION_SEND` — to Bluetooth, and to whatever the
 * manufacturer added — so the chooser is real but may hold one entry or none
 * depending on the watch. That is why the module asks the device instead of
 * deciding from the platform, and why `canShare()` exists: ask it, and hide the
 * button when the answer is `false`.
 */
export type SharePlatform = Exclude<NativePlatform, 'tvos' | 'watchos'>

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
   * Whether there is anywhere to share to.
   *
   * On tvOS and watchOS it is the platform answering: there is no sheet. On
   * Wear OS it is the watch answering, because what a Wear watch can receive
   * varies with the watch. Ask before showing the button; `share()` rejects by
   * name when the answer is `false`.
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
