import type { Metadata } from 'next';
import { Fraunces, Hanken_Grotesk, JetBrains_Mono } from 'next/font/google';
import { Providers } from './providers';
import './globals.css';

const display = Fraunces({
  subsets: ['latin'],
  weight: ['400', '500', '600', '900'],
  style: ['normal', 'italic'],
  variable: '--font-display',
  display: 'swap',
});

const body = Hanken_Grotesk({
  subsets: ['latin'],
  weight: ['400', '500', '600', '700'],
  variable: '--font-body',
  display: 'swap',
});

const mono = JetBrains_Mono({
  subsets: ['latin'],
  weight: ['400', '500', '700'],
  variable: '--font-mono',
  display: 'swap',
});

export const metadata: Metadata = {
  metadataBase: new URL('https://chamber.tasty.limo'),
  title: 'The Chamber — author OpenGov referenda together',
  description:
    'Collectively author, approve, and submit OpenGov referenda on Polkadot Hub.',
  // openGraph/twitter inherit title & description from the base metadata above;
  // twitter also inherits images from openGraph and defaults card → summary_large_image.
  openGraph: {
    type: 'website',
    url: 'https://chamber.tasty.limo',
    siteName: 'The Chamber',
    images: [
      {
        url: '/og.jpg',
        width: 2200,
        height: 1150,
        alt: 'The Chamber — author and submit OpenGov referenda together.',
      },
    ],
  },
  twitter: {
    card: 'summary_large_image',
  },
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${display.variable} ${body.variable} ${mono.variable}`}>
      <body>
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
