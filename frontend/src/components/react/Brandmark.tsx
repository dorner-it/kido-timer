// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

interface Props {
  size?: 'sm' | 'md' | 'lg';
}

const SIZES: Record<NonNullable<Props['size']>, { box: string; text: string; dot: string }> = {
  sm: { box: 'h-8 w-8 rounded-lg', text: 'text-[14px]', dot: 'h-1.5 w-1.5' },
  md: { box: 'h-10 w-10 rounded-xl', text: 'text-[18px]', dot: 'h-2 w-2' },
  lg: { box: 'h-16 w-16 rounded-2xl', text: 'text-[28px]', dot: 'h-3 w-3' },
};

export function Brandmark({ size = 'md' }: Props) {
  const s = SIZES[size];
  return (
    <div
      className={`relative grid ${s.box} place-items-center bg-gradient-to-br from-signal to-status-captured shadow-[0_8px_24px_-8px_rgba(255,77,46,0.7)]`}
    >
      <span className={`font-stencil ${s.text} font-bold leading-none text-ink-900`}>KiDo</span>
      <span
        className={`absolute -bottom-0.5 -right-0.5 ${s.dot} rounded-full bg-status-confirmed ring-2 ring-ink-900`}
      />
    </div>
  );
}
