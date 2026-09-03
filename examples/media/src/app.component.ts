import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, SafeArea } from '@angular-native/primitives'

/**
 * Map and video.
 *
 * The map is `MKMapView` on iOS. Android has none in the platform, so
 * OpenStreetMap tiles are drawn onto a `Canvas`: native, but not the system
 * map. The video does belong to both: `AVPlayerLayer` and `VideoView`.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <an-view [style.width]="'100%'" [style.height]="'100%'" [backgroundColor]="'#0b1020'">
      <an-safe-area [style.flex]="1" [padding]="16" [style.gap]="'12'">
        <an-text [fontSize]="26" [fontWeight]="700" [color]="'#f8fafc'">map and video</an-text>

        <an-map-view
          [style.flex]="1"
          [borderRadius]="14"
          [latitude]="lat()"
          [longitude]="lon()"
          [zoom]="zoom()" />

        <an-view [style.flexDirection]="'row'" [style.gap]="'10'" [style.height]="44">
          <an-button
            [style.flexGrow]="'1'"
            [title]="'Mallorca'"
            [variant]="'tonal'"
            [color]="'#6ee7b7'"
            (press)="go(39.5696, 2.6502, 11)"></an-button>
          <an-button
            [style.flexGrow]="'1'"
            [title]="'Zoom in'"
            [variant]="'tonal'"
            [color]="'#6ee7b7'"
            (press)="zoom.set(zoom() + 1)"></an-button>
        </an-view>

        <an-video-view
          [style.height]="200"
          [borderRadius]="14"
          [backgroundColor]="'#000000'"
          [url]="video"
          [playing]="playing()"
          [muted]="true" />

        <an-button
          [style.height]="44"
          [title]="playing() ? 'Pause' : 'Play'"
          [variant]="'filled'"
          [color]="'#6ee7b7'"
          (press)="playing.set(!playing())"></an-button>
      </an-safe-area>
    </an-view>
  `
})
export class AppComponent {
  /**
   * Apple's test stream.
   *
   * It is HLS and not an MP4 because the iOS simulator does not decode the
   * sample MP4s people usually reach for: the player reports them as ready and
   * then shows the "this cannot be played" icon. The Android emulator decodes
   * neither of the two —it ships no codecs—, so the video is tested on iOS and
   * not on Android.
   */
  readonly video =
    'https://devstreaming-cdn.apple.com/videos/streaming/examples/img_bipbop_adv_example_ts/master.m3u8'

  readonly lat = signal(40.4168)
  readonly lon = signal(-3.7038)
  readonly zoom = signal(11)
  readonly playing = signal(false)

  go(lat: number, lon: number, zoom: number): void {
    this.lat.set(lat)
    this.lon.set(lon)
    this.zoom.set(zoom)
  }
}
