// @ts-check
import starlight from '@astrojs/starlight'
import { defineConfig } from 'astro/config'

// La documentación del proyecto.
//
// El contenido son ficheros Markdown y nada más: Astro los sirve, no los
// posee. Quien escribe una página no tiene que saber nada de Astro, y el día
// que este sitio se cambie por otro, lo escrito sigue valiendo.
//
// El inglés es el idioma por defecto y la fuente: es lo que ve quien llega sin
// pedir idioma, y es contra lo que se traduce. Starlight recae en él cuando una
// página no está traducida todavía, así que una traducción a medias enseña la
// página en inglés en vez de un 404 — que es justo lo que hace falta mientras
// una traducción se pone al día.
export default defineConfig({
  integrations: [
    starlight({
      title: 'angular-native',
      defaultLocale: 'root',
      locales: {
        root: { label: 'English', lang: 'en' },
        es: { label: 'Español', lang: 'es' }
      },
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/nesgarbo/angular-native'
        }
      ],
      sidebar: [
        {
          label: 'Start here',
          translations: { es: 'Empezar' },
          items: [{ autogenerate: { directory: 'start' } }]
        },
        {
          label: 'Platforms',
          translations: { es: 'Plataformas' },
          items: [{ autogenerate: { directory: 'platforms' } }]
        },
        {
          label: 'Accessibility',
          translations: { es: 'Accesibilidad' },
          items: [{ autogenerate: { directory: 'accessibility' } }]
        },
        {
          label: 'Extending it',
          translations: { es: 'Extenderlo' },
          items: [{ autogenerate: { directory: 'extending' } }]
        }
      ]
    })
  ]
})
