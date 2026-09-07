import { docsLoader, i18nLoader } from '@astrojs/starlight/loaders'
import { docsSchema, i18nSchema } from '@astrojs/starlight/schema'
import { defineCollection } from 'astro:content'
import { z } from 'astro:schema'

export const collections = {
  docs: defineCollection({
    loader: docsLoader(),
    // Three fields the home page's hero needs and Starlight's own hero does not
    // have. They live in frontmatter rather than in the component so that every
    // word on the page belongs to the page, and the Spanish home page is a
    // translation rather than a second component.
    schema: docsSchema({
      extend: z.object({
        // A line of facts above the headline: what it is, in numbers.
        eyebrow: z.string().optional(),
        // The one command that starts a project, shown with a copy button.
        command: z.string().optional(),
        // The caption under the code sample beside the headline.
        sample: z.string().optional(),
        // The line above the headline: what changed recently, and where it is
        // written up. Both optional — with no `announce` the pill is not drawn.
        announce: z.string().optional(),
        announceLink: z.string().optional()
      })
    })
  }),
  // Starlight's own interface strings. Its Spanish is complete, so this holds
  // only the handful of labels that are this site's rather than Starlight's.
  i18n: defineCollection({ loader: i18nLoader(), schema: i18nSchema() })
}
