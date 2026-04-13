import { useMemo } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Table, TableBody, TableCell, TableRow } from '@/components/ui/table';
import { getRulesInfo } from '@/hooks/use-wasm';

interface RulesInfoProps {
  rulesName: string;
}

export default function RulesInfo({ rulesName }: RulesInfoProps) {
  const info = useMemo(() => getRulesInfo(rulesName), [rulesName]);

  if (!info) return null;

  const rows: [string, string][] = [
    ['Grid', info.grid],
    ['Initial flips', String(info.initial_flips)],
    ['Deck size', String(info.deck_size)],
    ['End threshold', `\u2265 ${info.end_threshold}`],
    ['Discard piles', info.discard_piles],
    ['Column clear', `${info.column_clear} matching`],
    ['Going out penalty', info.going_out_penalty],
    ['Reshuffle on empty', info.reshuffle_on_empty ? 'Yes' : 'No'],
  ];

  return (
    <Card className="w-full lg:w-64 shrink-0">
      <CardHeader className="pb-2">
        <CardTitle className="text-sm">Rules: {rulesName}</CardTitle>
      </CardHeader>
      <CardContent className="p-0">
        <Table>
          <TableBody>
            {rows.map(([label, value]) => (
              <TableRow key={label}>
                <TableCell className="text-xs font-medium py-1.5 px-3">{label}</TableCell>
                <TableCell className="text-xs py-1.5 px-3 text-right">{value}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}
