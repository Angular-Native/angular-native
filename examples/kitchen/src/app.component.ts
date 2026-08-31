import { ChangeDetectionStrategy, Component, computed, signal } from '@angular/core'
import { NATIVE_PRIMITIVES, VirtualList } from '@angular-native/primitives'

interface Row {
  id: number
  name: string
}

/**
 * Lista de cinco mil filas con búsqueda. Sirve para ver dos cosas: que el
 * campo de texto escribe en una señal, y que la lista solo monta las filas
 * visibles por muchas que haya.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, VirtualList],
  template: `
    <View
      [style.width]="'100%'"
      [style.height]="'100%'"
      [style.paddingTop]="'64'"
      [backgroundColor]="'#0b1020'">

      <View [style.paddingHorizontal]="'16'" [style.gap]="'12'" [style.paddingBottom]="'12'">
        <Text [fontSize]="24" [fontWeight]="'bold'" [color]="'#f4f7ff'">
          {{ visible().length }} de {{ rows.length }}
        </Text>

        <TextInput
          [style.height]="'40'"
          [placeholder]="'filtrar…'"
          [value]="query()"
          [color]="'#f4f7ff'"
          [fontSize]="16"
          [backgroundColor]="'#1e2a4a'"
          [borderRadius]="8"
          (valueChange)="query.set($event)" />
      </View>

      <VirtualList [items]="visible()" [itemHeight]="56" [style.flexGrow]="'1'">
        <ng-template let-row let-index="index">
          <View
            [style.height]="'56'"
            [style.paddingHorizontal]="'16'"
            [style.justifyContent]="'center'"
            [backgroundColor]="index % 2 === 0 ? '#141c33' : '#0b1020'">
            <Text [fontSize]="16" [color]="'#9fb0d4'">{{ row.name }}</Text>
          </View>
        </ng-template>
      </VirtualList>
    </View>
  `
})
export class AppComponent {
  readonly rows: Row[] = Array.from({ length: 5000 }, (_, id) => ({
    id,
    name: `fila número ${id}`
  }))

  readonly query = signal('')

  readonly visible = computed(() => {
    const needle = this.query().trim().toLowerCase()
    if (!needle) return this.rows
    return this.rows.filter((row) => row.name.toLowerCase().includes(needle))
  })
}
