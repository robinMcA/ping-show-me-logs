import { Handle, type NodeProps, Position, type Node } from "@xyflow/react";

export type PingNode = Node<{ handles: Array<Handle>; name?: string }, "ping">;

export const PingNode = ({ data }: NodeProps<PingNode>) => {
  const handles = data.handles as Array<Handle>;

  return (
    <>
      <Handle
        type={"target"}
        position={Position.Left}
        className="custom-handle"
      />
      <div className="resizer-node__handles">
        {handles.map((hand, i) => (
          <Handle
            type={"source"}
            position={Position.Right}
            style={{
              top: (i + 1) * 20,
              background: "none",
            }}
            id={hand.id}
            key={`${i}-${hand.id}-${hand.nodeId}`}
            className="resizer-node__handle custom-handle"
          >
            {hand.id}
          </Handle>
        ))}
      </div>
      <div>{data?.name ?? "ToDo"}</div>
    </>
  );
};
