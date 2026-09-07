// @ts-check
import { readFileSync, writeFileSync } from 'node:fs'
import starlight from '@astrojs/starlight'
import { defineConfig } from 'astro/config'

// Where the site lives, read from the one file that says so.
//
// The URL is not only Astro's business: it is in the README's links, in the
// `homepage` of every npm package, and inside the error messages the CLI
// prints when it wants to send somebody to a page. Those cannot share a
// variable across three languages and a JSON manifest, so they are written out
// — and `scripts/check-docs-url.sh` fails the build when any of them disagrees
// with this file. Moving the site is editing `DOCS_URL` and running that
// script with `--fix`; nothing else is edited by hand.
const site = readFileSync(new URL('../DOCS_URL', import.meta.url), 'utf8').trim()

// GitHub Pages serves a custom domain only if the built output carries a
// `CNAME` file, and serves a `*.github.io` address only if it does not. Which
// of the two is wanted follows from the URL above, so it is derived rather
// than kept as a second switch somebody has to remember to flip.
const customDomain = new URL(site).hostname
const cname = customDomain.endsWith('.github.io') ? null : customDomain

/** @type {import('astro').AstroIntegration} */
const pagesCname = {
  name: 'an:pages-cname',
  hooks: {
    'astro:build:done': ({ dir }) => {
      if (cname) writeFileSync(new URL('CNAME', dir), cname + '\n')
    }
  }
}

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
  site,
  integrations: [
    pagesCname,
    starlight({
      title: 'angular-native',
      // A chevron turning into a filled rounded rectangle: markup on the left,
      // the control the system draws for it on the right. Two files rather than
      // one so the mark is legible on both backgrounds instead of being a grey
      // that is a compromise on each.
      logo: {
        light: './src/assets/logo-light.svg',
        dark: './src/assets/logo-dark.svg'
      },
      // The identity: one CSS file for the site and one for the home page. Both
      // are plain CSS over Starlight's own custom properties, which is the
      // supported way in, and neither fights the theme with `!important`.
      customCss: ['./src/styles/theme.css', './src/styles/landing.css'],
      // Starlight's `Hero` is a title and two buttons; this page has to make an
      // argument in the first screenful, so that one component is replaced and
      // everything else is left alone.
      components: {
        Hero: './src/components/Hero.astro',
        // The stock page head is a title with the description under it as
        // prose. These pages needed to say where in the site they sit, and to
        // open the way the home page's sections do.
        PageTitle: './src/components/PageTitle.astro'
      },
      // Code is most of what is read here, so it gets a theme of its own rather
      // than the default's high-contrast primaries: Vitesse is low in
      // saturation and sits next to the vermilion instead of arguing with it.
      // The surfaces are overridden to the site's own so a code block reads as
      // part of the page rather than a window pasted onto it.
      expressiveCode: {
        themes: ['vitesse-dark', 'vitesse-light'],
        styleOverrides: {
          borderRadius: '0.625rem',
          borderColor: 'var(--an-border)',
          codeBackground: 'var(--an-code-bg)',
          codeFontFamily: 'var(--sl-font-mono)',
          codeFontSize: '0.8438rem',
          codeLineHeight: '1.65',
          codePaddingBlock: '0.9rem',
          codePaddingInline: '1.1rem',
          frames: {
            editorTabBarBackground: 'var(--an-code-bar)',
            editorTabBarBorderBottomColor: 'var(--an-border)',
            editorActiveTabBackground: 'var(--an-code-bg)',
            editorActiveTabBorderColor: 'var(--an-border)',
            editorActiveTabIndicatorTopColor: 'var(--sl-color-accent)',
            editorActiveTabForeground: 'var(--sl-color-white)',
            editorTabBorderRadius: '0.375rem',
            terminalBackground: 'var(--an-code-bg)',
            terminalTitlebarBackground: 'var(--an-code-bar)',
            terminalTitlebarBorderBottomColor: 'var(--an-border)',
            terminalTitlebarForeground: 'var(--sl-color-gray-3)',
            terminalTitlebarDotsForeground: 'var(--an-border-strong)',
            terminalTitlebarDotsOpacity: '1',
            inlineButtonBorder: 'var(--an-border-strong)',
            inlineButtonForeground: 'var(--sl-color-gray-2)',
            shadowColor: 'transparent'
          }
        }
      },
      // The tab colour a mobile browser paints its own chrome with. Without it
      // the bar stays white above a dark page, which is the one part of the
      // site the CSS cannot reach any other way.
      head: [
        {
          tag: 'meta',
          attrs: { name: 'theme-color', content: '#12151a', media: '(prefers-color-scheme: dark)' }
        },
        {
          tag: 'meta',
          attrs: { name: 'theme-color', content: '#ffffff', media: '(prefers-color-scheme: light)' }
        }
      ],
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
