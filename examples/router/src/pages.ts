import { ChangeDetectionStrategy, Component, inject, input, numberAttribute } from '@angular/core'
import { Router } from '@angular/router'
import { NATIVE_PRIMITIVES } from '@angular-native/primitives'

const SHIPS = [
  { id: 1, name: 'Sirena', port: 'Ibiza' },
  { id: 2, name: 'Levante', port: 'Formentera' },
  { id: 3, name: 'Tramontana', port: 'Palma' }
]

@Component({
  selector: 'page-home',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view [style.gap]="'12'" [style.padding]="'16'" [backgroundColor]="'#0b1020'"
          [style.width]="'100%'" [style.height]="'100%'">
      <an-text [fontSize]="24" [fontWeight]="'bold'" [color]="'#f4f7ff'">Barcos</an-text>
      @for (ship of ships; track ship.id) {
        <an-view
          [style.padding]="'16'"
          [backgroundColor]="'#1e2a4a'"
          [borderRadius]="10"
          (press)="open(ship.id)">
          <an-text [fontSize]="17" [color]="'#f4f7ff'">{{ ship.name }}</an-text>
        </an-view>
      }
    </an-view>
  `
})
export class HomePage {
  private readonly router = inject(Router)
  readonly ships = SHIPS

  open(id: number): void {
    void this.router.navigate(['/barco', id])
  }
}

@Component({
  selector: 'page-detail',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES],
  template: `
    <an-view [style.gap]="'12'" [style.padding]="'16'" [backgroundColor]="'#141c33'"
          [style.width]="'100%'" [style.height]="'100%'">
      <an-view [style.padding]="'12'" [backgroundColor]="'#2b1e4a'" [borderRadius]="8" (press)="back()">
        <an-text [fontSize]="15" [color]="'#c4b5fd'">‹ Volver</an-text>
      </an-view>
      <an-text [fontSize]="28" [fontWeight]="'bold'" [color]="'#f4f7ff'">{{ ship()?.name }}</an-text>
      <an-text [fontSize]="16" [color]="'#9fb0d4'">Puerto base: {{ ship()?.port }}</an-text>
    </an-view>
  `
})
export class DetailPage {
  private readonly router = inject(Router)

  /** Llega de la ruta gracias a `withComponentInputBinding()`. */
  readonly id = input(0, { transform: numberAttribute })

  ship = () => SHIPS.find((ship) => ship.id === this.id())

  back(): void {
    void this.router.navigate(['/'])
  }
}
