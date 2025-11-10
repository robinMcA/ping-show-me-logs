import {
  ReactFlow,
  ReactFlowProvider,
  useOnSelectionChange,
} from "@xyflow/react";
import { useState } from "react";
import useSWR from "swr";
import { PingNode } from "./custom/CustomNodes.tsx";
import { Log } from "./custom/Logs.tsx";
import { jsonFetcher } from "./helpers.ts";
import "./Flow.css";

const nodeTypes = {
  ping: PingNode,
};

const FlowInner = () => {
  const [startsWith, setStartsWith] = useState<string>("");
  const [endsWith, setEndsWith] = useState<string>("");

  const urlSearch = new URLSearchParams({
    starts_with: startsWith,
    ends_with: endsWith,
  });
  const { data: journeyList } = useSWR(
    `${document.URL.includes("5173") ? "http://localhost:8081" : ""}/api/journey?${urlSearch.toString()}`,
    jsonFetcher,
  );
  const [selectedJourney, setReselectedJourney] = useState<string>();

  const [selectedNode, setSelectedNode] = useState<string | undefined>(
    undefined,
  );
  const [transactionId, setTransactionId] = useState<string | undefined>(
    undefined,
  );

  useOnSelectionChange({
    onChange: (data) => {
      const node = data.nodes[0];

      if (!node) {
        return;
      }

      setSelectedNode(node.id as string);
    },
  });

  const { data: journeyFlow } = useSWR(
    selectedJourney === undefined
      ? null
      : `${document.URL.includes("5173") ? "http://localhost:8081" : ""}/api/journey/${selectedJourney}/flow${transactionId !== undefined ? `?transaction_id=${transactionId}` : ""}`,
    jsonFetcher,
  );

  const { data: journeyTransactions } = useSWR(
    selectedJourney === undefined
      ? null
      : `${document.URL.includes("5173") ? "http://localhost:8081" : ""}/api/journey/${selectedJourney}/transactions`,
    (url: string) =>
      jsonFetcher(url).then(
        (
          data: {
            transaction_id: string;
            timestamp: string;
          }[],
        ) => [
          ...new Set(
            data
              .sort(({ timestamp: timestampA }, { timestamp: timestampB }) =>
                timestampA > timestampB ? 1 : -1,
              )
              .map(({ transaction_id }) =>
                transaction_id.split("-request")[0].replace(/\/\d/gu, ""),
              ),
          ),
        ],
      ),
  );

  const { data: journeyScripts } = useSWR(
    selectedJourney === undefined
      ? null
      : `${document.URL.includes("5173") ? "http://localhost:8081" : ""}/api/journey/${selectedJourney}/scripts`,
    jsonFetcher,
  );

  console.info({ journeyScripts });

  const { data: scriptLogs } = useSWR(
    !journeyScripts || !selectedNode || !transactionId
      ? null
      : `${document.URL.includes("5173") ? "http://localhost:8081" : ""}/api/logs/${transactionId}?script_id=${journeyScripts?.[selectedNode]?.find((obj: Record<string, string>) => obj["type"] === "Scirpt")?._id}&script_name=${journeyScripts?.[selectedNode]?.find((obj: Record<string, string>) => obj["type"] === "Scirpt")?.name}`,
    jsonFetcher,
  );

  console.info(scriptLogs);

  const nodes = journeyFlow?.nodes.map(
    (node: { id: string; data: { name?: string }; handles: object[] }) => ({
      ...node,
      type: "ping",
      style: {
        height: Math.max(80, node.handles.length * 20 + 20),
      },
      data: {
        handles: node.handles,
        ...node.data,
        scriptContent: journeyScripts?.[node.id] ?? [{}, { script: "" }],
        name: node.data.name?.startsWith("s")
          ? node.data.name
          : node.data.name === "70e691a5-1e33-4ac3-a356-e7b6d60d92e0"
            ? "Success"
            : node.data.name === "e301438c-0bd0-429c-ab0c-66126501069a"
              ? "Fail"
              : node.data.name,
      },
    }),
  );

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const edges = journeyFlow?.edges.map((e: any) => ({
    ...e,
    markerEnd: "arrow",
    type: "simplebezier",
    animated: e.style.stroke !== "grey",
    style: {
      ...e.style,
      strokeWidth: e.style.stroke !== "grey" ? 10 : undefined,
    },
  }));

  return (
    <div style={{ display: "flex", flexDirection: "row" }}>
      <div style={{ minWidth: "650px" }}>
        <select
          value={transactionId}
          onChange={(event) => setTransactionId(event.target.value)}
        >
          {(journeyTransactions ?? []).map((transactionId, i) => (
            <option key={i} value={transactionId}>
              {transactionId}
            </option>
          ))}
        </select>
        <div style={{ padding: "30px" }}>
          {scriptLogs &&
            scriptLogs.result
              .filter(
                (res: unknown) =>
                  !JSON.stringify(res).includes("Unknown outcome"),
              )
              .map((log: { payload: Record<string, unknown> }, ix: number) => (
                <Log key={ix} data={log.payload} />
              ))}
        </div>
      </div>
      <div>
        <select
          value={selectedJourney}
          onChange={(event) => setReselectedJourney(event.target.value)}
        >
          {((journeyList as string[]) ?? []).sort().map((name, i) => (
            <option key={i} value={name}>
              {name}
            </option>
          ))}
        </select>
        <label htmlFor="startsWith">Starts With:</label>
        <input
          type="text"
          id="startsWith"
          name="startsWith"
          value={startsWith}
          onChange={(e) => setStartsWith(e.target.value)}
        />
        <label htmlFor="endsWith">Ends With:</label>
        <input
          type="text"
          id="endsWith"
          name="endsWith"
          value={endsWith}
          onChange={(e) => setEndsWith(e.target.value)}
        />
        <div style={{ height: "90vh", width: "90vw" }}>
          {journeyScripts && (
            <ReactFlow nodes={nodes} edges={edges} nodeTypes={nodeTypes} />
          )}
        </div>
      </div>
    </div>
  );
};

const Flow = () => (
  <ReactFlowProvider>
    <FlowInner />
  </ReactFlowProvider>
);
export default Flow;
