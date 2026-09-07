import { inject, Injectable } from '@angular/core'
import { NativeModules } from '@angular-native/platform'

/** A photograph that now exists on disk. */
export interface Photo {
  /**
   * Where it is. On Android from the library this is a `content://` URI and not
   * a path — that is what the photo picker hands over, the file may not be on
   * the device at all, and a `/`-path invented for it would point at nothing.
   * Pass it straight to `an-image`'s `source`, or to `Files.read`.
   */
  path: string
  width: number
  height: number
}

/** What the person has decided about this app and the camera. */
export type CameraPermission = 'granted' | 'denied' | 'prompt' | 'restricted'

export interface PhotoOptions {
  /**
   * The longest side, in pixels. The image is scaled down before it is written,
   * which is nearly always what an app wants: a modern phone camera produces
   * twelve megapixels, and uploading that to resize it on a server is the
   * expensive way round.
   *
   * Zero keeps the original.
   */
  maxSize?: number
  /** JPEG quality, 0 to 1. `0.85` by default. */
  quality?: number
}

/**
 * The camera, and the photo library.
 *
 * Both are **presented**, not embedded: `UIImagePickerController` and
 * `PHPickerViewController` on iOS, the system camera intent and the photo
 * picker on Android. A live preview inside your own layout would be a view, and
 * a plugin contributes methods rather than views — that boundary is described in
 * [Plugins](/extending/plugins/).
 *
 * The photograph is written to the app's own cache directory and its path comes
 * back. Nothing crosses the bridge as bytes: a twelve-megapixel image as base64
 * is sixteen megabytes of string through a JSON boundary, once to encode and
 * once to parse.
 *
 * ```ts
 * const camera = inject(Camera)
 * if (await camera.request() === 'granted') {
 *   const photo = await camera.takePhoto({ maxSize: 1600 })
 *   this.preview.set(photo.path)
 * }
 * ```
 */
@Injectable({ providedIn: 'root' })
export class Camera {
  private readonly modules = inject(NativeModules)

  /** What the person has already decided, without asking them again. */
  permission(): Promise<CameraPermission> {
    return this.modules.call<CameraPermission>('camera', 'permission')
  }

  /** Asks, if there is anything to ask, and waits for the answer. */
  request(): Promise<CameraPermission> {
    return this.modules.call<CameraPermission>('camera', 'request')
  }

  /**
   * Opens the camera and resolves with what was taken.
   *
   * Cancelling is not an error and not a rejection you have to parse: it
   * rejects with a message that says the person cancelled, so the two cases —
   * "they changed their mind" and "the camera is broken" — do not look the same
   * to an app that only catches.
   */
  takePhoto(options: PhotoOptions = {}): Promise<Photo> {
    return this.modules.call<Photo>('camera', 'takePhoto', options)
  }

  /**
   * Opens the photo library.
   *
   * On iOS 14 and later this needs **no permission at all**: `PHPickerViewController`
   * runs outside the app and hands back only what was chosen, so there is
   * nothing to ask for. On Android 13 and later the photo picker works the same
   * way. Asking for library access on those is asking for something the system
   * already refuses to make relevant.
   */
  pickPhoto(options: PhotoOptions = {}): Promise<Photo> {
    return this.modules.call<Photo>('camera', 'pickPhoto', options)
  }
}
