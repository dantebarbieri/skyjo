import { useEffect } from 'react';

const SITE_NAME = 'Skyjo';

export function useDocumentTitle(page?: string) {
  useEffect(() => {
    const previous = document.title;
    document.title = page ? `${page} | ${SITE_NAME}` : SITE_NAME;
    return () => {
      document.title = previous;
    };
  }, [page]);
}
