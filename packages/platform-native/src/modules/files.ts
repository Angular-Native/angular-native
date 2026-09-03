import { inject, Injectable } from '@angular/core'

import { NativeModules, type NativePlatform } from '../native-modules'

/** One entry of a directory, or one file the person picked. */
export interface FileEntry {
  /** The last component of the path, or what the picker called it. */
  name: string
  /**
   * What to hand back to `read`.
   *
   * Inside the app's own directories it is a path. For something that came out
   * of `pick()` on Android it is a `content://` URI instead: that is what the
   * Storage Access Framework gives, the document may not even be on the device,
   * and a `/`-path invented for it would be a path to nothing.
   */
  path: string
  /** In bytes. `0` for a directory, and `0` for a picked file whose provider would not say. */
  size: number
  isDirectory: boolean
}

/** What `pick()` may be narrowed by. */
export interface PickOptions {
  /**
   * What may be chosen. Uniform type identifiers on Apple's platforms
   * (`'public.image'`), MIME types on Android (`'image/*'`). Empty means
   * anything.
   */
  types?: string[]
  /** Whether more than one file may be chosen. `false` by default. */
  multiple?: boolean
}

/**
 * Where the system file picker exists.
 *
 * Not on a television and not on a watch: neither ships a document browser, and
 * `UIDocumentPickerViewController` does not exist on either. On Wear OS the
 * intent resolves to nothing, which the module reports as the same absence.
 * `pick()` rejects there saying which platform it is and what to do instead.
 */
export type FilePickerPlatform = Exclude<NativePlatform, 'tvos' | 'watchos' | 'wearos'>

/**
 * Where storage that survives the app being closed exists.
 *
 * Everywhere except tvOS, which gives an app no persistent local storage at
 * all: what there is is a cache the system empties whenever it wants the room.
 * `documentsDirectory()` rejects there rather than answering a path whose
 * contents disappear without warning.
 */
export type DurableStoragePlatform = Exclude<NativePlatform, 'tvos'>

/**
 * The app's files.
 *
 * Two things, and the difference between them is the whole module:
 *
 * - **What the app owns.** `documentsDirectory()` and `cacheDirectory()`, which
 *   are inside the app's container on every platform. Nothing here asks for a
 *   permission, because an app does not need one to write in its own directory.
 * - **What the person chose.** `pick()`, which is the system's own picker —
 *   `UIDocumentPickerViewController`, `NSOpenPanel`, `ACTION_OPEN_DOCUMENT` —
 *   and the only way to reach anything outside the container.
 *
 * The module enforces the line between the two rather than documenting it: it
 * writes only inside the app's directories, and outside them it reads only what
 * came back from `pick()`, for as long as the app is running. A path that is
 * neither is rejected saying which of the two it failed. Without that, a path
 * arriving from JS would be a path to anywhere on the disk.
 *
 * A relative path is resolved against `documentsDirectory()`, so
 * `read('notes.txt')` works without asking where that is first — except on
 * tvOS, where there is no such directory and it lands in the cache. That is the
 * only place a television lets an app write, and it is a session's worth of
 * storage rather than none.
 */
@Injectable({ providedIn: 'root' })
export class Files {
  private readonly modules = inject(NativeModules)

  /**
   * The directory that lasts. `Documents` in the container on iOS, iPadOS,
   * visionOS and watchOS; `Application Support/<bundle id>` on macOS, which is
   * where a Mac app's own files belong — `~/Documents` is the person's folder
   * and needs their consent to touch; `getFilesDir()` on Android and Wear OS.
   *
   * On tvOS it rejects: see `DurableStoragePlatform`.
   */
  documentsDirectory(): Promise<string> {
    return this.modules.call<string>('files', 'documentsDirectory')
  }

  /** The directory the system may empty when it needs the room. Everywhere. */
  cacheDirectory(): Promise<string> {
    return this.modules.call<string>('files', 'cacheDirectory')
  }

  /** Whether there is anything there. */
  exists(path: string): Promise<boolean> {
    return this.modules.call<boolean>('files', 'exists', { path })
  }

  /** The contents, as UTF-8 text. */
  read(path: string): Promise<string> {
    return this.modules.call<string>('files', 'read', { path })
  }

  /** Writes UTF-8 text, creating the directories above it if they are missing. */
  write(path: string, text: string): Promise<void> {
    return this.modules.call<void>('files', 'write', { path, text })
  }

  /** Deletes a file, or a directory and everything in it. */
  remove(path: string): Promise<void> {
    return this.modules.call<void>('files', 'remove', { path })
  }

  /** Creates a directory, and the ones above it if they are missing. */
  makeDirectory(path: string): Promise<void> {
    return this.modules.call<void>('files', 'makeDirectory', { path })
  }

  /** What is in a directory, sorted by name. */
  list(path: string): Promise<FileEntry[]> {
    return this.modules.call<FileEntry[]>('files', 'list', { path })
  }

  /**
   * Opens the system picker and waits.
   *
   * An empty array means the person closed it without choosing, which is not a
   * failure and does not reject. Only what comes back from here may be read
   * from outside the app's own directories.
   */
  pick(options: PickOptions = {}): Promise<FileEntry[]> {
    return this.modules.call<FileEntry[]>('files', 'pick', options)
  }
}
