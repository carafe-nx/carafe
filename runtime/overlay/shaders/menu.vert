#version 450

layout(location = 0) out vec2 coord;

void main()
{
    vec2 corner = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2);
    coord = corner;
    gl_Position = vec4(corner * 2.0 - 1.0, 0.0, 1.0);
}
