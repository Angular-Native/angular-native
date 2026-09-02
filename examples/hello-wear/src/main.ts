import { bootstrapNativeApplication } from '@angular-native/platform'

import { AppComponent } from './app.component'

bootstrapNativeApplication(AppComponent).catch((error) => {
  console.error('el arranque falló:', error)
})
