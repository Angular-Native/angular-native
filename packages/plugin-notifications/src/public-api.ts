import { inject, Injectable } from '@angular/core'
import { NativeModules } from '@angular-native/platform'

/** What the person has decided about this app and notifications. */
export type NotificationPermission = 'granted' | 'denied' | 'prompt'

export interface LocalNotification {
  /** Yours. Scheduling twice with the same id replaces the first. */
  id: string
  title: string
  body?: string
  /**
   * When to show it. A `Date`, or milliseconds since the epoch. Leave it out and
   * it is delivered immediately — which on both platforms means *now*, even if
   * the app is in front.
   */
  at?: Date | number
  /** Anything you want back when it is tapped. It travels as JSON. */
  data?: Record<string, unknown>
}

/** A notification the person tapped, or that arrived while the app was open. */
export interface NotificationEvent {
  id: string
  title: string
  body: string
  data: Record<string, unknown>
  /** `true` when the person tapped it, `false` when it merely arrived. */
  tapped: boolean
}

/**
 * Notifications the app schedules itself.
 *
 * `UNUserNotificationCenter` on the Apple platforms and `NotificationManager` on
 * Android. This is **local** notifications only: something the app decides to
 * show, at a time it chooses, with no server involved.
 *
 * Remote notifications are a different thing wearing the same word. They need a
 * server, a certificate on Apple's side and a Firebase project on Google's, and
 * the part a framework can offer — the token, and the event when one is tapped —
 * is in `remoteToken()` and the `notification` event below. Everything past that
 * is your backend's.
 *
 * ```ts
 * const notifications = inject(Notifications)
 * if (await notifications.request() === 'granted') {
 *   await notifications.schedule({
 *     id: 'kettle',
 *     title: 'The kettle',
 *     body: 'Three minutes are up.',
 *     at: Date.now() + 180_000
 *   })
 * }
 *
 * const stop = notifications.on((event) => {
 *   if (event.tapped) this.router.navigate(['/timer', event.id])
 * })
 * ```
 */
@Injectable({ providedIn: 'root' })
export class Notifications {
  private readonly modules = inject(NativeModules)

  /** What the person has already decided, without asking them again. */
  permission(): Promise<NotificationPermission> {
    return this.modules.call<NotificationPermission>('notifications', 'permission')
  }

  /**
   * Asks, if there is anything to ask, and waits for the answer.
   *
   * On Android this is only a dialog from API 33: before that the permission is
   * granted at install time and this resolves `granted` without showing
   * anything.
   */
  request(): Promise<NotificationPermission> {
    return this.modules.call<NotificationPermission>('notifications', 'request')
  }

  /** Schedules one. Scheduling with an id that is already pending replaces it. */
  schedule(notification: LocalNotification): Promise<void> {
    const at = notification.at instanceof Date ? notification.at.getTime() : notification.at
    return this.modules.call<void>('notifications', 'schedule', { ...notification, at })
  }

  /** Cancels one that has not fired yet. Cancelling an unknown id is not an error. */
  cancel(id: string): Promise<void> {
    return this.modules.call<void>('notifications', 'cancel', { id })
  }

  /** The ids of everything scheduled and not yet delivered. */
  pending(): Promise<string[]> {
    return this.modules.call<string[]>('notifications', 'pending')
  }

  /** Clears what is already sitting in the notification centre. */
  clearDelivered(): Promise<void> {
    return this.modules.call<void>('notifications', 'clearDelivered')
  }

  /**
   * The token this device is reachable at, for remote notifications.
   *
   * On Apple it is the APNs device token as hexadecimal. On Android it is the
   * FCM registration token, and it needs Firebase in the app — without it this
   * rejects saying so rather than returning an empty string you would send to a
   * server that could never use it.
   *
   * The token changes. Send it to your backend every launch, not once.
   */
  remoteToken(): Promise<string> {
    return this.modules.call<string>('notifications', 'remoteToken')
  }

  /**
   * Notifications arriving or being tapped.
   *
   * A tap is the interesting one: it is how an app opens the screen the
   * notification was about, and it can arrive before the app has finished
   * starting. The one that was tapped to launch the app is held and delivered
   * as soon as something subscribes, so a handler registered in a component's
   * constructor does not miss it.
   *
   * @returns the unsubscriber. Call it.
   */
  on(handler: (event: NotificationEvent) => void): () => void {
    const off = this.modules.on<NotificationEvent>('notifications', 'notification', handler)
    // Subscribing is also what releases the tap that launched the app. The
    // native side has no way of asking whether anybody is listening yet, so it
    // holds taps until the first subscriber says it is ready — which is here.
    void this.modules.call<void>('notifications', 'drain').catch(() => {
      // A platform without the plugin has already rejected louder elsewhere.
    })
    return off
  }
}
