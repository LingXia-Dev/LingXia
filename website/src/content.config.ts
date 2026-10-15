import { defineCollection } from 'astro:content';
import { docsLoader, i18nLoader } from '@astrojs/starlight/loaders';
import { docsSchema, i18nSchema } from '@astrojs/starlight/schema';

// Starlight docs collection. Marketing pages live in src/pages and are unaffected.
export const collections = {
  docs: defineCollection({ loader: docsLoader(), schema: docsSchema() }),
  // Starlight UI string overrides; an empty file keeps its built-in zh-CN.
  i18n: defineCollection({ loader: i18nLoader(), schema: i18nSchema() }),
};
