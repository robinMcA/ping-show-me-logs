type LogProps = { data: Record<string, unknown> };

export const Log = ({ data }: LogProps) => (
  <>
    <p>
      <b>{String(data.level)}</b> {String(data.message)}
    </p>
  </>
);
