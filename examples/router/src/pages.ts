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
    <View [style.gap]="'12'" [style.padding]="'16'">
      <Text [fontSize]="24" [fontWeight]="'bold'" [color]="'#f4f7ff'">Barcos</Text>
      @for (ship of ships; track ship.id) {
        <View
          [style.padding]="'16'"
          [backgroundColor]="'#1e2a4a'"
          [borderRadius]="10"
          (press)="open(ship.id)">
          <Text [fontSize]="17" [color]="'#f4f7ff'">{{ ship.name }}</Text>
        </View>
      }
    </View>
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
    <View [style.gap]="'12'" [style.padding]="'16'">
      <View [style.padding]="'12'" [backgroundColor]="'#2b1e4a'" [borderRadius]="8" (press)="back()">
        <Text [fontSize]="15" [color]="'#c4b5fd'">‹ Volver</Text>
      </View>
      <Text [fontSize]="28" [fontWeight]="'bold'" [color]="'#f4f7ff'">{{ ship()?.name }}</Text>
      <Text [fontSize]="16" [color]="'#9fb0d4'">Puerto base: {{ ship()?.port }}</Text>
    </View>
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
