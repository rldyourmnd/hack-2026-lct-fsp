import {
  type Edge,
  Handle,
  type Node,
  type NodeProps,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { useMemo } from 'react';
import { type Graph, skillStatusView } from '@/entities/skill';
import { Button, cx } from '@/shared/ui';
import styles from './competence-graph.module.css';
import { layoutGraph } from './layout';

type GraphNode = Graph['nodes'][number];
type NodeData = { node: GraphNode; selected: boolean; dimmed: boolean; onSelect: (id: string) => void };
type FlowNode = Node<NodeData, 'competence'>;

const kindLabel: Record<GraphNode['kind'], string> = {
  role: 'Категория',
  concept: 'Понятие',
  skill: 'Технология',
  source: 'Источник',
  evidence: 'Свидетельство',
};

function CompetenceNode({ data }: NodeProps<FlowNode>) {
  const { node, selected, dimmed, onSelect } = data;
  const status = node.status ? skillStatusView[node.status] : null;
  return (
    <>
      <Handle type="target" position={Position.Top} className={styles.handle} isConnectable={false} />
      <button
        type="button"
        className={cx(styles.node, styles[node.kind], dimmed && styles.dimmed)}
        aria-pressed={selected}
        aria-label={`${kindLabel[node.kind]}: ${node.label}${node.detail ? `, ${node.detail}` : ''}${status ? `, ${status.label}` : ''}`}
        onClick={() => onSelect(node.id)}
      >
        {(node.kind === 'source' || node.kind === 'concept') && (
          <span className={cx(styles.meta, 'muted')}>{kindLabel[node.kind]}</span>
        )}
        <span className={styles.label}>{node.label}</span>
        {node.detail && <span className={styles.meta}>{node.detail}</span>}
        {status && node.kind === 'skill' && <span className={cx(styles.meta, 'muted')}>{status.label}</span>}
        {node.kind === 'evidence' && <span className={cx(styles.meta, 'muted')}>Свидетельство</span>}
      </button>
      <Handle type="source" position={Position.Bottom} className={styles.handle} isConnectable={false} />
    </>
  );
}

const nodeTypes = { competence: CompetenceNode };

const edgeColor: Record<Graph['edges'][number]['relation'], string> = {
  uses: 'var(--fsp-color-border-strong)',
  supports: 'var(--fsp-color-primary)',
  requires: 'var(--fsp-color-accent)',
};

function Toolbar() {
  const flow = useReactFlow();
  return (
    <div className={styles.toolbar}>
      <Button size="sm" icon="minus" aria-label="Уменьшить" onClick={() => void flow.zoomOut()} />
      <Button size="sm" icon="plus" aria-label="Увеличить" onClick={() => void flow.zoomIn()} />
      <Button
        size="sm"
        icon="fit"
        aria-label="Показать весь граф"
        onClick={() => void flow.fitView({ padding: 0.2 })}
      />
    </div>
  );
}

export type CompetenceGraphProps = {
  graph: Graph;
  selectedId: string | null;
  highlight?: (node: GraphNode) => boolean;
  onSelect: (nodeId: string) => void;
  label: string;
};

/** Renders a server graph; it never creates, removes or re-weights nodes or edges. */
export default function CompetenceGraph({ graph, selectedId, highlight, onSelect, label }: CompetenceGraphProps) {
  const { nodes, edges } = useMemo(() => {
    const positions = layoutGraph(graph);
    const flowNodes: FlowNode[] = graph.nodes.map((node) => ({
      id: node.id,
      type: 'competence',
      position: positions.get(node.id) ?? { x: 0, y: 0 },
      data: { node, selected: node.id === selectedId, dimmed: highlight ? !highlight(node) : false, onSelect },
      draggable: false,
      selectable: false,
      connectable: false,
    }));
    const flowEdges: Edge[] = graph.edges.map((edge) => ({
      id: edge.id,
      source: edge.from,
      target: edge.to,
      type: 'smoothstep',
      selectable: false,
      focusable: false,
      style: { stroke: edgeColor[edge.relation], strokeWidth: 1.5 },
    }));
    return { nodes: flowNodes, edges: flowEdges };
  }, [graph, selectedId, highlight, onSelect]);

  return (
    <section className={styles.frame} aria-label={label}>
      <ReactFlowProvider>
        <Toolbar />
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          fitView
          fitViewOptions={{ padding: 0.2 }}
          minZoom={0.3}
          maxZoom={1.25}
          nodesDraggable={false}
          nodesConnectable={false}
          nodesFocusable={false}
          edgesFocusable={false}
          elementsSelectable={false}
          // Nodes without click handling are made inert by React Flow; this keeps pointer selection working.
          onNodeClick={(_, node) => onSelect(node.id)}
          zoomOnScroll={false}
          preventScrolling={false}
          proOptions={{ hideAttribution: false }}
        />
      </ReactFlowProvider>
    </section>
  );
}
