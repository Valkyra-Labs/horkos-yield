export declare const RELATIVE: number;

export type Report = { failures: string[]; worst: number; worstAt: string };

export declare function decodeNaN(v: unknown): unknown;

export declare function compare(got: unknown, want: unknown, path?: string, report?: Report): Report;
