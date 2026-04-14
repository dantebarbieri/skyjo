import { useState, useEffect, useMemo } from 'react';
import { useParams, useNavigate, Link } from 'react-router-dom';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import {
  Collapsible,
  CollapsibleTrigger,
  CollapsibleContent,
} from '@/components/ui/collapsible';
import { Separator } from '@/components/ui/separator';
import { cn } from '@/lib/utils';
import { useDocumentTitle } from '@/hooks/use-document-title';
import { getStrategyDescriptions } from '@/hooks/use-wasm';
import { useWasmContext } from '@/contexts/wasm-context';
import type {
  StrategyDescriptionsData,
  StrategyDescription,
  DecisionLogic,
  DecisionNode,
  PriorityRule,
  CommonConcept,
} from '@/types';

const COMPLEXITY_COLORS: Record<string, string> = {
  Trivial: 'bg-muted text-muted-foreground',
  Low: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200',
  Medium:
    'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200',
  High: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200',
};

// --- Decision Logic Renderers ---

function SimpleLogic({ text }: { text: string }) {
  return (
    <p className="italic text-muted-foreground text-sm pl-2 border-l-2 border-muted">
      {text}
    </p>
  );
}

function PriorityRuleItem({
  rule,
  index,
}: {
  rule: PriorityRule;
  index: number;
}) {
  const content = (
    <div className="flex gap-2 items-start">
      <Badge
        variant="outline"
        className="shrink-0 mt-0.5 size-5 justify-center rounded-full p-0 text-[10px] font-semibold"
      >
        {index + 1}
      </Badge>
      <div className="min-w-0">
        <span className="font-medium text-sm">{rule.condition}</span>
        <span className="text-muted-foreground text-sm"> → </span>
        <span className="text-sm">{rule.action}</span>
      </div>
    </div>
  );

  if (!rule.detail) {
    return <li className="py-1">{content}</li>;
  }

  return (
    <li className="py-1">
      <Collapsible>
        <CollapsibleTrigger className="w-full text-left group cursor-pointer">
          <div className="flex items-start gap-1">
            <div className="flex-1">{content}</div>
            <span className="text-muted-foreground text-xs shrink-0 mt-1 group-data-[state=open]:rotate-90 transition-transform">
              ▸
            </span>
          </div>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <p className="text-xs text-muted-foreground mt-1 ml-7">
            {rule.detail}
          </p>
        </CollapsibleContent>
      </Collapsible>
    </li>
  );
}

function PriorityListLogic({ rules }: { rules: PriorityRule[] }) {
  return (
    <ol className="space-y-0.5 list-none p-0">
      {rules.map((rule, i) => (
        <PriorityRuleItem key={i} rule={rule} index={i} />
      ))}
    </ol>
  );
}

function DecisionTreeLogic({ node, depth = 0 }: { node: DecisionNode; depth?: number }) {
  if (node.type === 'Action') {
    return (
      <div className="flex items-start gap-2">
        <span className="text-primary font-medium text-sm shrink-0">→</span>
        <div>
          <span className="text-sm">{node.action}</span>
          {node.detail && (
            <Collapsible>
              <CollapsibleTrigger className="text-xs text-muted-foreground cursor-pointer hover:text-foreground ml-1">
                [details]
              </CollapsibleTrigger>
              <CollapsibleContent>
                <p className="text-xs text-muted-foreground mt-1">
                  {node.detail}
                </p>
              </CollapsibleContent>
            </Collapsible>
          )}
        </div>
      </div>
    );
  }

  if (node.type === 'PriorityList') {
    return <PriorityListLogic rules={node.rules} />;
  }

  // Condition node
  return (
    <div className={cn('space-y-1', depth > 0 && 'ml-4 pl-3 border-l-2 border-border')}>
      <div className="flex items-start gap-2">
        <Badge variant="outline" className="shrink-0 mt-0.5 text-[10px] px-1.5 py-0">
          IF
        </Badge>
        <span className="text-sm font-medium">{node.test}</span>
      </div>
      <div className="ml-4 pl-3 border-l-2 border-green-300 dark:border-green-700 space-y-1">
        <span className="text-xs font-semibold text-green-700 dark:text-green-400">
          YES
        </span>
        <DecisionTreeLogic node={node.if_true} depth={depth + 1} />
      </div>
      <div className="ml-4 pl-3 border-l-2 border-red-300 dark:border-red-700 space-y-1">
        <span className="text-xs font-semibold text-red-700 dark:text-red-400">
          NO
        </span>
        <DecisionTreeLogic node={node.if_false} depth={depth + 1} />
      </div>
    </div>
  );
}

function LogicRenderer({ logic }: { logic: DecisionLogic }) {
  switch (logic.type) {
    case 'Simple':
      return <SimpleLogic text={logic.text} />;
    case 'PriorityList':
      return <PriorityListLogic rules={logic.rules} />;
    case 'DecisionTree':
      return <DecisionTreeLogic node={logic.root} />;
  }
}

// --- Phase Section ---

const PHASE_LABELS: Record<string, string> = {
  InitialFlips: '🂠 Initial Flips',
  ChooseDraw: 'Draw Decision',
  DeckDrawAction: 'After Drawing from Deck',
  DiscardDrawPlacement: 'After Drawing from Discard',
};

function PhaseSection({
  phase,
}: {
  phase: { phase: string; label: string; logic: DecisionLogic };
}) {
  return (
    <div className="space-y-2">
      <h4 className="text-sm font-semibold text-muted-foreground uppercase tracking-wide">
        {PHASE_LABELS[phase.phase] ?? phase.label}
      </h4>
      <LogicRenderer logic={phase.logic} />
    </div>
  );
}

// --- Strategy Detail ---

function StrategyDetail({
  strategy,
  concepts,
}: {
  strategy: StrategyDescription;
  concepts: CommonConcept[];
}) {
  const usedConcepts = useMemo(
    () =>
      strategy.concepts
        .map((ref) => {
          const concept = concepts.find((c) => c.id === ref.id);
          return concept ? { ...concept, used_for: ref.used_for } : null;
        })
        .filter(Boolean) as (CommonConcept & { used_for: string })[],
    [strategy.concepts, concepts]
  );

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <div className="flex items-center gap-3 mb-2">
          <h2 className="text-2xl font-bold">{strategy.name}</h2>
          <Badge className={cn('text-xs', COMPLEXITY_COLORS[strategy.complexity])}>
            {strategy.complexity}
          </Badge>
        </div>
        <p className="text-muted-foreground">{strategy.summary}</p>
      </div>

      {/* Strengths & Weaknesses */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <Card>
          <CardHeader className="pb-2 pt-3 px-4">
            <CardTitle className="text-sm font-semibold text-green-700 dark:text-green-400">
              Strengths
            </CardTitle>
          </CardHeader>
          <CardContent className="px-4 pb-3">
            <ul className="space-y-1">
              {strategy.strengths.map((s, i) => (
                <li key={i} className="text-sm flex gap-2">
                  <span className="text-green-600 dark:text-green-400 shrink-0">+</span>
                  {s}
                </li>
              ))}
            </ul>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2 pt-3 px-4">
            <CardTitle className="text-sm font-semibold text-red-700 dark:text-red-400">
              Weaknesses
            </CardTitle>
          </CardHeader>
          <CardContent className="px-4 pb-3">
            <ul className="space-y-1">
              {strategy.weaknesses.map((w, i) => (
                <li key={i} className="text-sm flex gap-2">
                  <span className="text-red-600 dark:text-red-400 shrink-0">−</span>
                  {w}
                </li>
              ))}
            </ul>
          </CardContent>
        </Card>
      </div>

      {/* Decision Phases */}
      <div className="space-y-5">
        <h3 className="text-lg font-semibold">Decision Logic</h3>
        {strategy.phases.map((phase) => (
          <PhaseSection key={phase.phase} phase={phase} />
        ))}
      </div>

      {/* Concepts Used */}
      {usedConcepts.length > 0 && (
        <div className="space-y-3">
          <h3 className="text-lg font-semibold">Concepts Used</h3>
          <div className="space-y-2">
            {usedConcepts.map((c) => (
              <div key={c.id} className="flex gap-2 items-start text-sm">
                <a
                  href={`#concept-${c.id}`}
                  className="shrink-0 font-medium text-primary hover:underline"
                >
                  {c.label}
                </a>
                <span className="text-muted-foreground">— {c.used_for}</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// --- Common Concepts Section ---

function CommonConceptsSection({ concepts }: { concepts: CommonConcept[] }) {
  return (
    <div className="space-y-4">
      <h3 className="text-lg font-semibold" id="common-concepts">
        Common Concepts
      </h3>
      <p className="text-sm text-muted-foreground">
        Shared techniques used by multiple strategies.
      </p>
      <div className="grid gap-3">
        {concepts.map((concept) => (
          <Card key={concept.id} id={`concept-${concept.id}`}>
            <CardHeader className="pb-2 pt-3 px-4">
              <CardTitle className="text-sm font-semibold">
                {concept.label}
              </CardTitle>
            </CardHeader>
            <CardContent className="px-4 pb-3 space-y-2">
              <p className="text-sm text-muted-foreground">
                {concept.description}
              </p>
              {concept.formula && (
                <code className="block text-xs bg-muted px-3 py-2 rounded font-mono">
                  {concept.formula}
                </code>
              )}
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}

// --- Strategy Nav ---

function StrategyNav({
  strategies,
  active,
  onSelect,
}: {
  strategies: StrategyDescription[];
  active: string;
  onSelect: (name: string) => void;
}) {
  return (
    <>
      {/* Desktop: vertical sidebar */}
      <nav className="hidden lg:block sticky top-20 space-y-1 min-w-44">
        <h3 className="text-xs font-semibold text-muted-foreground uppercase tracking-wide mb-2 px-2">
          Strategies
        </h3>
        {strategies.map((s) => (
          <button
            key={s.name}
            onClick={() => onSelect(s.name)}
            className={cn(
              'block w-full text-left px-3 py-1.5 rounded-md text-sm transition-colors',
              active === s.name
                ? 'bg-primary/10 text-primary font-medium'
                : 'text-muted-foreground hover:text-foreground hover:bg-muted'
            )}
          >
            <div className="flex items-center justify-between">
              {s.name}
              <Badge
                className={cn(
                  'text-[9px] px-1 py-0 leading-tight',
                  COMPLEXITY_COLORS[s.complexity]
                )}
              >
                {s.complexity}
              </Badge>
            </div>
          </button>
        ))}
        <Separator className="my-3" />
        <a
          href="#common-concepts"
          className="block px-3 py-1.5 text-sm text-muted-foreground hover:text-foreground hover:bg-muted rounded-md transition-colors"
        >
          Common Concepts
        </a>
      </nav>
      {/* Mobile: horizontal scrollable tabs */}
      <nav className="lg:hidden flex gap-2 overflow-x-auto pb-2 -mx-1 px-1">
        {strategies.map((s) => (
          <button
            key={s.name}
            onClick={() => onSelect(s.name)}
            className={cn(
              'shrink-0 px-3 py-1.5 rounded-full text-sm border transition-colors',
              active === s.name
                ? 'bg-primary text-primary-foreground border-primary'
                : 'bg-background text-muted-foreground border-border hover:text-foreground'
            )}
          >
            {s.name}
          </button>
        ))}
      </nav>
    </>
  );
}

// --- Main Route ---

export default function StrategiesRoute() {
  useDocumentTitle('Strategy Guide');
  const { strategyName } = useParams();
  const navigate = useNavigate();
  useWasmContext(); // ensure WASM is loaded

  const [data, setData] = useState<StrategyDescriptionsData | null>(null);
  const [active, setActive] = useState<string>('');

  useEffect(() => {
    const result = getStrategyDescriptions();
    if (result) setData(result);
  }, []);

  useEffect(() => {
    if (!data) return;
    if (strategyName && data.strategies.some((s) => s.name === strategyName)) {
      setActive(strategyName);
    } else if (!active || !data.strategies.some((s) => s.name === active)) {
      setActive(data.strategies[0]?.name ?? '');
    }
  }, [data, strategyName, active]);

  const handleSelect = (name: string) => {
    setActive(name);
    navigate(`/rules/strategies/${name}`, { replace: true });
    // On mobile, scroll to top of content
    window.scrollTo({ top: 0, behavior: 'smooth' });
  };

  if (!data) {
    return (
      <div className="text-center text-muted-foreground py-12">
        Loading strategy descriptions...
      </div>
    );
  }

  const activeStrategy = data.strategies.find((s) => s.name === active);

  return (
    <div className="space-y-6">
      {/* Breadcrumb */}
      <div className="text-sm text-muted-foreground">
        <Link to="/rules" className="hover:text-foreground transition-colors">
          Rules
        </Link>
        <span className="mx-2">/</span>
        <span className="text-foreground">Strategy Guide</span>
      </div>

      <div className="flex gap-8">
        {/* Sidebar */}
        <StrategyNav
          strategies={data.strategies}
          active={active}
          onSelect={handleSelect}
        />

        {/* Main content */}
        <div className="flex-1 min-w-0 space-y-8">
          {activeStrategy && (
            <StrategyDetail
              strategy={activeStrategy}
              concepts={data.common_concepts}
            />
          )}

          <Separator />

          <CommonConceptsSection concepts={data.common_concepts} />
        </div>
      </div>
    </div>
  );
}
