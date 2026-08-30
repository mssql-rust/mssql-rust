// Constants shared by the client-side components. Keep this free of any
// content imports so it stays cheap to ship to the browser.

export const SITE_URL = 'https://mssql-rust.github.io';
export const SITE_NAME = 'mssql';
export const SITE_TAGLINE = 'A Rust client for Microsoft SQL Server';

export const REPOSITORY = 'https://github.com/mssql-rust/mssql-rust';
export const GITLAB = 'https://gitlab.com/mssql-rust/mssql-rust';
export const CODEBERG = 'https://codeberg.org/mssql-rust/mssql-rust';
export const CRATES_IO = 'https://crates.io/crates/mssql';
export const DOCS_RS = 'https://docs.rs/mssql';
export const TIBERIUS = 'https://github.com/prisma/tiberius';

/** Themes vendored into static/themes/ by bin/sync-lily.mjs. */
export const THEMES = ['light', 'dark', 'nord', 'dracula'];

export const THEME_LABELS = {
	light: 'Light',
	dark: 'Dark',
	nord: 'Nord',
	dracula: 'Dracula'
};
