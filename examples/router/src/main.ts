import { provideRouter, withComponentInputBinding } from '@angular/router'
import {
  bootstrapNativeApplication,
  NATIVE_LOCATION_PROVIDERS,
  NATIVE_STACK_PROVIDERS
} from '@angular-native/platform'

import { AppComponent } from './app.component'
import { DetailPage, HomePage } from './pages'

bootstrapNativeApplication(AppComponent, {
  providers: [
    ...NATIVE_LOCATION_PROVIDERS,
    ...NATIVE_STACK_PROVIDERS,
    provideRouter(
      [
        { path: '', component: HomePage },
        { path: 'barco/:id', component: DetailPage }
      ],
      withComponentInputBinding()
    )
  ]
}).catch((error) => {
  console.error('el arranque falló:', error)
})
