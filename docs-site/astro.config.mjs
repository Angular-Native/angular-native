// @ts-check
import starlight from '@astrojs/starlight'
import { defineConfig } from 'astro/config'

// The project's documentation.
//
// The content is Markdown files and nothing else: Astro serves them, it does
// not own them. Whoever writes a page has to know nothing about Astro, and the
// day this site is swapped for another one, what was written still stands.
//
// English is the default language and the source: it is what somebody who
// arrives without asking for a language sees, and it is what translations are
// made against. Starlight falls back to it when a page has not been translated
// yet, so a half-finished translation shows the English page instead of a 404 —
// which is exactly what is wanted while a translation catches up.
export default defineConfig({
  site: 'https://angular-native.dev',
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
      // Five groups and not ten: somebody arriving wants to learn, somebody
      // already here wants to look something up, somebody porting to a platform
      // wants to know what is missing there, and accessibility and extending it
      // are each a subject of their own. A sidebar mirroring the file tree would
      // make the reader work that difference out.
      //
      // Order inside each group comes from `sidebar.order` in every page's
      // frontmatter: alphabetical would put the quick start after the guide it
      // is the entrance to.
      sidebar: [
        {
          label: 'Guide',
          translations: { es: 'Guía' },
          items: [{ autogenerate: { directory: 'guide' } }]
        },
        {
          label: 'Reference',
          translations: { es: 'Referencia' },
          items: [{ autogenerate: { directory: 'reference' } }]
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
