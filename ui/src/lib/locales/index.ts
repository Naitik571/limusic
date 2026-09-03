import { en, type Translations } from './en';
import { tr } from './tr';

/** A catalog that has not been fully translated yet: every key optional, all the way down. */
type DeepPartial<T> = { [K in keyof T]?: T[K] extends object ? DeepPartial<T[K]> : T[K] };

export type LocaleId = 'en' | 'tr';

export interface LocaleInfo {
	id: LocaleId;
	label: string;
	nativeLabel: string;
	flag?: string;
}

export const LOCALES: LocaleInfo[] = [
	{ id: 'en', label: 'English', nativeLabel: 'English' },
	{ id: 'tr', label: 'Turkish', nativeLabel: 'Türkçe' }
];

export const translations: Record<LocaleId, DeepPartial<Translations>> = {
	en,
	tr
};

export { en, tr };
export type { Translations };
