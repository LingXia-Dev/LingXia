import { defineCollection } from 'astro:content';
import { docsLoader } from '@astrojs/starlight/loaders';
import { docsSchema } from '@astrojs/starlight/schema';

// Starlight docs collection. Marketing pages live in src/pages and are unaffected.
export const collections = {
  docs: defineCollection({ loader: docsLoader(), schema: docsSchema() }),
};
