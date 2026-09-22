// Deliberately fixed presentation fixtures, not generated source or compiler output.
pub(crate) const TITLE: &str = "谱与振动：从能量到特征值";
pub(crate) const ABSTRACT: &str = "振动问题把几何、能量与线性代数联系起来。本文从一根固定两端的弦出发，讨论驻波的形成，并用 Rayleigh 商刻画系统的固有频率。";
pub(crate) const INTRO: &str = "设弦的长度为 L，张力为 T，线密度为 ρ。微小位移 u(x, t) 满足波动方程。将空间与时间变量分离，可以把动力学问题化为一个边值问题。";
pub(crate) const BOUNDARY: &str = "固定端点给出边界条件 X(0) = X(L) = 0。只有离散的一组频率能同时满足方程与边界条件；它们对应不同阶的振动模态。";
pub(crate) const ENERGY: &str = "对于满足边界条件的非零函数，势能与动能之比给出了特征值的变分描述。第一特征值对应所有可容许振型中的最小值。";
pub(crate) const CONCLUSION: &str = "这一观点不依赖某个特定坐标系。由弦到薄膜，再到一般的弹性结构，能量方法为分析与数值近似提供了统一的起点。";

pub(crate) const LATEX: &str = r"\documentclass[11pt]{article}
\usepackage{amsmath}
\usepackage{ctex}

\title{谱与振动：从能量到特征值}
\author{Scholium}
\date{}

\begin{document}
\maketitle

\begin{abstract}
振动问题把几何、能量与线性代数联系起来。
本文从一根固定两端的弦出发，讨论驻波的形成，
并用 Rayleigh 商刻画系统的固有频率。
\end{abstract}

\section{弦的固有模态}
设弦的长度为 $L$，张力为 $T$，线密度为 $\rho$。
微小位移 $u(x,t)$ 满足波动方程。
\begin{equation}
  \frac{\partial^2 u}{\partial t^2}
  = c^2 \frac{\partial^2 u}{\partial x^2},
  \qquad c = \sqrt{T/\rho}.
\end{equation}
固定端点给出 $X(0)=X(L)=0$。

\section{能量与 Rayleigh 商}
势能与动能之比给出特征值的变分描述：
\[
  R[X] = \frac{\int_0^L T |X'(x)|^2\,dx}
               {\int_0^L \rho |X(x)|^2\,dx}.
\]
由弦到薄膜，能量方法提供了统一的起点。
\end{document}";

pub(crate) const TYPST: &str = r#"#set page(paper: "a4", margin: 24mm)
#set text(lang: "zh", size: 11pt)
#set heading(numbering: "1")

#align(center)[
  #text(size: 22pt)[谱与振动：从能量到特征值]
  #v(8pt)
  Scholium
]

*摘要* \
振动问题把几何、能量与线性代数联系起来。
本文从一根固定两端的弦出发，讨论驻波的形成，
并用 Rayleigh 商刻画系统的固有频率。

= 弦的固有模态
设弦的长度为 $L$，张力为 $T$，线密度为 $rho$。
微小位移 $u(x,t)$ 满足波动方程。

$ frac(partial^2 u, partial t^2)
  = c^2 frac(partial^2 u, partial x^2),
  quad c = sqrt(T/rho) $

固定端点给出 $X(0)=X(L)=0$。

= 能量与 Rayleigh 商
势能与动能之比给出特征值的变分描述：

$ R[X] = frac(integral_0^L T abs(X'(x))^2 dif x,
              integral_0^L rho abs(X(x))^2 dif x) $

由弦到薄膜，能量方法提供了统一的起点。
"#;
