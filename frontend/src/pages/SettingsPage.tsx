import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { getLlmConfig, saveLlmConfig, clearLlmConfig } from "@/lib/api";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";

export function SettingsPage() {
  const queryClient = useQueryClient();

  // ---------- LLM 配置 ----------
  const { data: llmConfig, isLoading: llmLoading } = useQuery({
    queryKey: ["llm-config"],
    queryFn: getLlmConfig,
    staleTime: 0,
  });

  const [apiBase, setApiBase] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("");
  const [initialized, setInitialized] = useState(false);

  // 只在首次加载时初始化表单值
  if (llmConfig && !initialized) {
    setApiBase(llmConfig.api_base);
    setModel(llmConfig.model);
    setInitialized(true);
  }

  const saveMutation = useMutation({
    mutationFn: (input: { api_base?: string; api_key?: string; model?: string }) =>
      saveLlmConfig(input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["llm-config"] });
    },
  });

  const clearMutation = useMutation({
    mutationFn: clearLlmConfig,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["llm-config"] });
      setApiKey("");
      setInitialized(false);
    },
  });

  const handleSave = () => {
    saveMutation.mutate({
      api_base: apiBase || undefined,
      api_key: apiKey || undefined,
      model: model || undefined,
    });
  };

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="max-w-2xl mx-auto p-6 space-y-8">
        <div>
          <h1 className="text-2xl font-bold">设置</h1>
          <p className="text-sm text-muted-foreground mt-1">
            管理应用配置，修改后自动保存。
          </p>
        </div>

        {/* LLM 配置 */}
        <Card>
          <CardHeader>
            <CardTitle>AI 编排助手</CardTitle>
            <CardDescription>
              配置 LLM API 以启用 AI 编排助手。API Key 保存在本地数据库中，不会上传到任何第三方。
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {llmLoading ? (
              <p className="text-sm text-muted-foreground">加载中...</p>
            ) : (
              <>
                <div className="grid gap-2">
                  <Label htmlFor="api-base">API Base URL</Label>
                  <Input
                    id="api-base"
                    value={apiBase}
                    onChange={(e) => setApiBase(e.target.value)}
                    placeholder="https://api.openai.com/v1"
                  />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="api-key">
                    API Key
                    {llmConfig?.configured && (
                      <span className="text-xs text-muted-foreground ml-2">（已配置，留空则不变）</span>
                    )}
                  </Label>
                  <Input
                    id="api-key"
                    type="password"
                    value={apiKey}
                    onChange={(e) => setApiKey(e.target.value)}
                    placeholder={llmConfig?.configured ? "••••••••" : "sk-..."}
                  />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="model">Model</Label>
                  <Input
                    id="model"
                    value={model}
                    onChange={(e) => setModel(e.target.value)}
                    placeholder="gpt-4o"
                  />
                </div>

                <div className="flex items-center gap-3 pt-2">
                  <Button
                    onClick={handleSave}
                    disabled={saveMutation.isPending}
                    size="sm"
                  >
                    {saveMutation.isPending ? "保存中..." : "保存"}
                  </Button>
                  {saveMutation.isSuccess && (
                    <span className="text-xs text-green-600">已保存</span>
                  )}
                  {saveMutation.isError && (
                    <span className="text-xs text-destructive">保存失败</span>
                  )}
                  {llmConfig?.configured && (
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => clearMutation.mutate()}
                      disabled={clearMutation.isPending}
                    >
                      清除配置
                    </Button>
                  )}
                </div>
              </>
            )}
          </CardContent>
        </Card>

        {/* 执行器配置 */}
        <Card>
          <CardHeader>
            <CardTitle>执行器</CardTitle>
            <CardDescription>
              配置 coding agent 的执行参数。
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid gap-2">
              <Label>可用 Agent</Label>
              <p className="text-sm text-muted-foreground">
                系统会自动发现 PATH 中已安装的 Claude Code 和 OpenCode。如需切换默认 agent，请查看文档。
              </p>
            </div>
          </CardContent>
        </Card>

        {/* 关于 */}
        <Card>
          <CardHeader>
            <CardTitle>关于</CardTitle>
            <CardDescription>Vibe Dashboard 版本与应用信息</CardDescription>
          </CardHeader>
          <CardContent className="space-y-2 text-sm text-muted-foreground">
            <p>Vibe Dashboard — 本地 AI 编程管理工具</p>
            <p>
              技术栈：Rust (Axum + SQLx + SQLite) · React + TypeScript + shadcn/ui
            </p>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}