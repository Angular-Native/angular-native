import { ChangeDetectionStrategy, Component, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, SafeArea } from '@angular-native/primitives'

/**
 * Mapa y vídeo.
 *
 * El mapa es `MKMapView` en iOS. En Android no hay ninguno en la plataforma,
 * así que se dibujan teselas de OpenStreetMap sobre un `Canvas`: nativo, pero
 * no el mapa del sistema. El vídeo sí es de los dos: `AVPlayerLayer` y
 * `VideoView`.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, SafeArea],
  template: `
    <View [style.width]="'100%'" [style.height]="'100%'" [backgroundColor]="'#0b1020'">
      <SafeArea [style.flex]="1" [padding]="16" [style.gap]="'12'">
        <Text [fontSize]="26" [fontWeight]="700" [color]="'#f8fafc'">mapa y vídeo</Text>

        <MapView
          [style.flex]="1"
          [borderRadius]="14"
          [latitude]="lat()"
          [longitude]="lon()"
          [zoom]="zoom()" />

        <View [style.flexDirection]="'row'" [style.gap]="'10'" [style.height]="44">
          <Button
            [style.flexGrow]="'1'"
            [title]="'Mallorca'"
            [variant]="'tonal'"
            [color]="'#6ee7b7'"
            (press)="ir(39.5696, 2.6502, 11)"></Button>
          <Button
            [style.flexGrow]="'1'"
            [title]="'Acercar'"
            [variant]="'tonal'"
            [color]="'#6ee7b7'"
            (press)="zoom.set(zoom() + 1)"></Button>
        </View>

        <VideoView
          [style.height]="200"
          [borderRadius]="14"
          [backgroundColor]="'#000000'"
          [url]="video"
          [playing]="reproduciendo()"
          [muted]="true" />

        <Button
          [style.height]="44"
          [title]="reproduciendo() ? 'Pausa' : 'Reproducir'"
          [variant]="'filled'"
          [color]="'#6ee7b7'"
          (press)="reproduciendo.set(!reproduciendo())"></Button>
      </SafeArea>
    </View>
  `
})
export class AppComponent {
  /**
   * El flujo de prueba de Apple.
   *
   * Es HLS y no un MP4 porque el simulador de iOS no decodifica los MP4 de
   * ejemplo que se suelen usar: el reproductor los da por listos y luego
   * enseña el icono de "esto no se puede ver". El emulador de Android no
   * decodifica ninguna de las dos cosas —no trae códecs—, así que el vídeo
   * está probado en iOS y no en Android.
   */
  readonly video =
    'https://devstreaming-cdn.apple.com/videos/streaming/examples/img_bipbop_adv_example_ts/master.m3u8'

  readonly lat = signal(40.4168)
  readonly lon = signal(-3.7038)
  readonly zoom = signal(11)
  readonly reproduciendo = signal(false)

  ir(lat: number, lon: number, zoom: number): void {
    this.lat.set(lat)
    this.lon.set(lon)
    this.zoom.set(zoom)
  }
}
