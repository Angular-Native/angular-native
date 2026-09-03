import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core'
import { Device } from '@angular-native/platform'
import { NATIVE_PRIMITIVES, VirtualList } from '@angular-native/primitives'

interface Row {
  id: number
  name: string
}

/**
 * A five thousand row list with a search field. It is there to show two things:
 * that the text field writes into a signal, and that the list only mounts the
 * visible rows however many there are.
 */
@Component({
  selector: 'app-root',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [NATIVE_PRIMITIVES, VirtualList],
  template: `
    <an-view
      [style.width]="'100%'"
      [style.height]="'100%'"
      [style.paddingTop]="'64'"
      [backgroundColor]="'#0b1020'">

      <an-view [style.paddingHorizontal]="'16'" [style.gap]="'12'" [style.paddingBottom]="'12'">
        <an-text [fontSize]="24" [fontWeight]="'bold'" [color]="'#f4f7ff'">
          {{ visible().length }} of {{ rows.length }}
        </an-text>

        <an-text [fontSize]="13" [color]="'#6ee7b7'">{{ deviceLabel() }}</an-text>

        <an-text-input
          [style.height]="'40'"
          [placeholder]="'filter…'"
          [placeholderColor]="'#6b7a99'"
          [value]="query()"
          [color]="'#f4f7ff'"
          [fontSize]="16"
          [fontWeight]="'500'"
          [keyboardType]="'default'"
          [returnKeyType]="'search'"
          [autoCapitalize]="'none'"
          [autoCorrect]="false"
          [textAlign]="'left'"
          [ios]="{ clearButtonMode: 'whileEditing' }"
          [android]="{ selectAllOnFocus: true }"
          [backgroundColor]="'#1e2a4a'"
          [borderRadius]="8"
          (valueChange)="query.set($event)" />
      </an-view>

      <an-virtual-list
        [items]="visible()"
        [itemHeight]="rowHeight"
        [style.flexGrow]="'1'"
        [refreshing]="reloading()"
        (refresh)="reload()">
        <ng-template let-row let-index="index">
          <an-view
            [style.height]="rowHeight(row)"
            [style.paddingHorizontal]="'16'"
            [style.justifyContent]="'center'"
            [backgroundColor]="index % 2 === 0 ? '#141c33' : '#0b1020'">
            <an-text [fontSize]="16" [color]="'#9fb0d4'">{{ row.name }}</an-text>
          </an-view>
        </ng-template>
      </an-virtual-list>
    </an-view>
  `
})
export class AppComponent {
  readonly rows: Row[] = Array.from({ length: 5000 }, (_, id) => ({
    id,
    name: `row number ${id}`
  }))

  /**
   * One row in five is taller. The list does not need them all to measure the
   * same: it is enough for it to know how tall each one is.
   */
  readonly rowHeight = (row: Row): number => (row.id % 5 === 0 ? 88 : 56)

  readonly query = signal('')
  readonly reloading = signal(false)

  /** Comes from a native module: the call does not block and lands in another frame. */
  private readonly device = signal<string | null>(null)
  readonly deviceLabel = computed(() => this.device() ?? 'asking the device…')

  constructor() {
    inject(Device)
      .info()
      .then((info) => this.device.set(`${info.platform} ${info.systemVersion} · ${info.locale}`))
      .catch((error: unknown) => this.device.set(`no device data: ${error}`))
  }

  /** Pull to refresh: a round trip to the server is faked. */
  reload(): void {
    this.reloading.set(true)
    setTimeout(() => this.reloading.set(false), 1200)
  }

  readonly visible = computed(() => {
    const needle = this.query().trim().toLowerCase()
    if (!needle) return this.rows
    return this.rows.filter((row) => row.name.toLowerCase().includes(needle))
  })
}
