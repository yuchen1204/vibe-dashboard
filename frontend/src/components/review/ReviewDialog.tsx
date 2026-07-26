import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogClose,
} from "@/components/ui/dialog";
import { useReviewDetail, useReviewsByTodo } from "@/hooks/useReview";
import { useReviewStore } from "@/stores/review";
import type { ReviewFinding } from "@/types/api";

interface Props {
  todoId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const SEVERITY_COLORS: Record<string, string> = {
  critical: "bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300",
  major: "bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-300",
  minor: "bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300",
  suggestion: "bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-300",
};

const SEVERITY_LABEL: Record<string, string> = {
  critical: "严重",
  major: "主要",
  minor: "次要",
  suggestion: "建议",
};

const STATUS_LABEL: Record<string, string> = {
  pending: "待审查",
  in_progress: "审查中",
  completed: "已完成",
  failed: "失败",
};

export function ReviewDialog({ todoId, open, onOpenChange }: Props) {
  const { data: reviews } = useReviewsByTodo(todoId);
  const [selectedReviewId, setSelectedReviewId] = useState<string | null>(null);

  const latestReview = reviews?.[0];
  const displayReviewId = selectedReviewId ?? latestReview?.id ?? null;
  const { data: reviewDetail } = useReviewDetail(displayReviewId);

  // 从 WS 实时 store 获取审查进度
  const reviewByTodo = useReviewStore((s) => s.reviewByTodo);
  const liveReviewId = reviewByTodo[todoId];
  const liveReview = useReviewStore((s) => (liveReviewId ? s.reviews[liveReviewId] : undefined));

  // 如果审查正在运行中，合并实时 findings
  const mergedFindings = (() => {
    if (liveReview?.status === "running" && liveReview.findings.length > 0) {
      const existingIds = new Set(reviewDetail?.findings.map((f) => f.id) ?? []);
      const newFindings = liveReview.findings.filter((f) => !existingIds.has(f.id));
      if (newFindings.length > 0) {
        return [...(reviewDetail?.findings ?? []), ...newFindings];
      }
    }
    return reviewDetail?.findings;
  })();

  const isRunning = liveReview?.status === "running";

  if (!reviews || reviews.length === 0) {
    return (
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>审查记录</DialogTitle>
          </DialogHeader>
          <div className="py-8 text-center text-sm text-muted-foreground">
            {isRunning ? (
              <div className="space-y-2">
                <p className="animate-pulse">AI 审查中...</p>
                {liveReview && liveReview.findings.length > 0 && (
                  <p className="text-xs">已发现 {liveReview.findings.length} 个问题</p>
                )}
              </div>
            ) : (
              "暂无审查记录"
            )}
          </div>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">关闭</Button>
            </DialogClose>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[80vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>
            审查记录
            {reviewDetail && (
              <span className="ml-2 text-sm font-normal text-muted-foreground">
                [{STATUS_LABEL[reviewDetail.status] ?? reviewDetail.status}]
                {reviewDetail.score != null && (
                  <span className="ml-2">评分: {reviewDetail.score}/10</span>
                )}
              </span>
            )}
            {isRunning && (
              <span className="ml-2 text-sm font-normal text-yellow-600 animate-pulse">
                · 审查中...
              </span>
            )}
          </DialogTitle>
        </DialogHeader>

        {/* Review selector */}
        {reviews.length > 1 && (
          <div className="flex gap-2 mb-2 overflow-x-auto pb-1">
            {reviews.map((r) => (
              <Button
                key={r.id}
                variant={r.id === displayReviewId ? "default" : "outline"}
                size="sm"
                className="text-xs shrink-0"
                onClick={() => setSelectedReviewId(r.id)}
              >
                {new Date(r.created_at).toLocaleDateString("zh-CN")}
                {r.status === "completed" && " ✓"}
              </Button>
            ))}
          </div>
        )}

        <div className="flex-1 overflow-y-auto space-y-4">
          {/* Summary */}
          {reviewDetail?.summary && (
            <div className="bg-muted rounded-lg p-4">
              <h4 className="text-sm font-semibold mb-1">审查总结</h4>
              <p className="text-sm text-muted-foreground whitespace-pre-wrap">
                {reviewDetail.summary}
              </p>
            </div>
          )}

          {/* Live summary from WS */}
          {isRunning && liveReview?.summary && !reviewDetail?.summary && (
            <div className="bg-muted rounded-lg p-4">
              <h4 className="text-sm font-semibold mb-1">审查总结（生成中）</h4>
              <p className="text-sm text-muted-foreground whitespace-pre-wrap">
                {liveReview.summary}
              </p>
            </div>
          )}

          {/* Findings count */}
          {reviewDetail && (
            <div className="flex items-center gap-3 text-sm">
              <span className="text-muted-foreground">
                共 {reviewDetail.total_findings} 个发现
                {isRunning && liveReview && liveReview.findings.length > reviewDetail.total_findings && (
                  <span className="text-yellow-600">（实时 {liveReview.findings.length} 个）</span>
                )}
              </span>
              {reviewDetail.score != null && (
                <span className="text-muted-foreground">
                  · 评分 {reviewDetail.score}/10
                </span>
              )}
            </div>
          )}

          {/* Running progress */}
          {isRunning && !reviewDetail && (
            <div className="text-sm text-muted-foreground space-y-1">
              <p className="animate-pulse">AI 正在审查代码...</p>
              {liveReview && (
                <p className="text-xs">已发现 {liveReview.findings.length} 个问题</p>
              )}
            </div>
          )}

          {/* Findings list (merged with live) */}
          {(mergedFindings ?? reviewDetail?.findings)?.map((f) => (
            <FindingCard key={f.id} finding={f} />
          ))}

          {reviewDetail && reviewDetail.findings.length === 0 && reviewDetail.status === "completed" && (
            <div className="py-8 text-center text-sm text-muted-foreground">
              未发现任何问题 ✓
            </div>
          )}
        </div>

        <DialogFooter>
          <DialogClose asChild>
            <Button variant="outline">关闭</Button>
          </DialogClose>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function FindingCard({ finding }: { finding: ReviewFinding }) {
  const severityClass = SEVERITY_COLORS[finding.severity] ?? SEVERITY_COLORS.minor;
  const severityLabel = SEVERITY_LABEL[finding.severity] ?? finding.severity;

  return (
    <div className="border rounded-lg p-4 space-y-2">
      <div className="flex items-start justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0">
          <span className={`text-xs font-medium px-2 py-0.5 rounded-full shrink-0 ${severityClass}`}>
            {severityLabel}
          </span>
          <span className="text-xs text-muted-foreground shrink-0">
            {finding.category}
          </span>
          <h4 className="text-sm font-medium truncate">{finding.title}</h4>
        </div>
      </div>

      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <code className="bg-muted px-1.5 py-0.5 rounded text-xs">{finding.file_path}</code>
        {finding.line_number && (
          <span>: {finding.line_number}</span>
        )}
      </div>

      {finding.description && (
        <p className="text-sm text-muted-foreground">{finding.description}</p>
      )}

      {finding.suggestion && (
        <div className="bg-green-50 dark:bg-green-950 border border-green-200 dark:border-green-800 rounded p-3">
          <p className="text-xs font-medium text-green-700 dark:text-green-400 mb-1">建议</p>
          <p className="text-sm text-green-600 dark:text-green-300">{finding.suggestion}</p>
        </div>
      )}
    </div>
  );
}