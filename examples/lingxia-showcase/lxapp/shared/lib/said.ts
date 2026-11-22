import { t } from '@lingxia/view';

/** A status line Logic set: a `<page>.state.*` key follows a language switch; an error's own text shows as is. */
export const said = (text: string, args?: Readonly<Record<string, unknown>>) => (/^[a-z]+\.state\.[a-zA-Z.]+$/.test(text) ? (t as (key: string, args?: Readonly<Record<string, unknown>>) => string)(text, args) : text);
